import { HttpService } from '@nestjs/axios';
import { Injectable, Logger, OnModuleInit } from '@nestjs/common';
import { Interval } from '@nestjs/schedule';
import { SorobanRpc } from '@stellar/stellar-sdk';
import { firstValueFrom } from 'rxjs';
import { getRpcEndpoints } from '../config/config.service';

type CircuitState = 'closed' | 'open' | 'half_open';
type NodeName = 'primary' | 'fallback';

interface RpcNode {
  name: NodeName;
  url: string;
  client: SorobanRpc.Server;
  state: CircuitState;
  failures: boolean[];
  consecutiveFailures: number;
  openedAt: number;
  healthy: boolean;
  latencyMs: number;
  latestLedger: number;
  probeSuccess: boolean;
}

@Injectable()
export class HealthService implements OnModuleInit {
  private readonly logger = new Logger(HealthService.name);
  private readonly nodes: RpcNode[];
  private active: NodeName = 'primary';
  private failovers = 0;
  private probing = false;
  private readonly cooldownMs = 30_000;
  private readonly maxLedgerLag = 2;

  constructor(private readonly httpService: HttpService) {
    const { primary, fallback } = getRpcEndpoints();
    this.nodes = [this.createNode('primary', primary)];
    if (fallback) this.nodes.push(this.createNode('fallback', fallback));
  }

  private createNode(name: NodeName, url: string): RpcNode {
    return {
      name,
      url,
      client: new SorobanRpc.Server(url),
      state: 'closed',
      failures: [],
      consecutiveFailures: 0,
      openedAt: 0,
      healthy: true,
      latencyMs: 0,
      latestLedger: 0,
      probeSuccess: false,
    };
  }

  async onModuleInit(): Promise<void> {
    await this.probeAll();
  }

  @Interval(10_000)
  async probeAll(): Promise<void> {
    if (this.probing) return;
    this.probing = true;
    try {
      await Promise.all(this.nodes.map((node) => this.probe(node)));
      const latest = Math.max(
        ...this.nodes.filter((n) => n.probeSuccess).map((n) => n.latestLedger),
        0,
      );
      for (const node of this.nodes) {
        if (
          node.probeSuccess &&
          latest - node.latestLedger > this.maxLedgerLag
        ) {
          node.healthy = false;
          this.recordFailure(node, 'ledger_lag');
        }
      }
      this.selectActive();
    } finally {
      this.probing = false;
    }
  }

  private async probe(node: RpcNode): Promise<void> {
    if (node.state === 'open') {
      if (Date.now() - node.openedAt < this.cooldownMs) return;
      this.setState(node, 'half_open', 'cooldown_elapsed');
    }
    const started = Date.now();
    try {
      const response = await firstValueFrom(
        this.httpService.post<{
          result?: { status?: string; latestLedger?: number };
          error?: unknown;
        }>(
          node.url,
          { jsonrpc: '2.0', id: 1, method: 'getHealth' },
          { timeout: 5_000 },
        ),
      );
      const result = response.data?.result;
      if (
        response.status !== 200 ||
        response.data?.error ||
        result?.status !== 'healthy' ||
        typeof result.latestLedger !== 'number' ||
        !Number.isFinite(result.latestLedger)
      ) {
        throw new Error('unhealthy_rpc_response');
      }
      node.latencyMs = Date.now() - started;
      node.latestLedger = result.latestLedger;
      node.probeSuccess = true;
      node.healthy = true;
      this.recordSuccess(node);
    } catch {
      node.latencyMs = Date.now() - started;
      node.probeSuccess = false;
      node.healthy = false;
      this.recordFailure(node, 'probe_failed');
    }
  }

  private recordSuccess(node: RpcNode): void {
    node.failures.push(false);
    if (node.failures.length > 10) node.failures.shift();
    node.consecutiveFailures = 0;
    if (node.state === 'half_open') {
      this.setState(node, 'closed', 'recovery_probe');
    } else {
      this.tripIfNeeded(node, 'error_rate');
    }
  }

  private recordFailure(node: RpcNode, reason: string): void {
    node.failures.push(true);
    if (node.failures.length > 10) node.failures.shift();
    node.consecutiveFailures++;
    this.tripIfNeeded(node, reason);
  }

  private tripIfNeeded(node: RpcNode, reason: string): void {
    const errorRate =
      node.failures.filter(Boolean).length / node.failures.length;
    if (
      node.state === 'half_open' ||
      node.consecutiveFailures >= 3 ||
      (node.failures.length >= 5 && errorRate > 0.2)
    ) {
      this.setState(node, 'open', reason);
    }
  }

  private setState(node: RpcNode, state: CircuitState, reason: string): void {
    if (node.state === state) return;
    node.state = state;
    if (state === 'open') node.openedAt = Date.now();
    if (state === 'closed') node.failures = [];
    this.logger.warn(
      JSON.stringify({
        event: 'rpc_circuit_transition',
        node: node.name,
        state,
        reason,
      }),
    );
  }

  private usable(node: RpcNode): boolean {
    return node.state === 'closed' && node.healthy;
  }

  private selectActive(): void {
    const preferred =
      this.nodes.find((node) => node.name === 'primary' && this.usable(node)) ||
      this.nodes.find((node) => this.usable(node));
    if (preferred && preferred.name !== this.active) {
      this.active = preferred.name;
      this.failovers++;
      this.logger.warn(
        JSON.stringify({ event: 'rpc_failover', active: this.active }),
      );
    }
  }

  isAvailable(): boolean {
    return this.nodes.some((node) => this.usable(node));
  }

  getSnapshot() {
    return {
      active: this.active,
      available: this.isAvailable(),
      nodes: this.nodes.map(
        ({ name, state, healthy, latencyMs, latestLedger }) => ({
          name,
          state,
          healthy,
          latencyMs,
          latestLedger,
        }),
      ),
    };
  }

  /** Delegate each SDK call to the currently selected server. Retry read calls once on backup. */
  getRpcServer(): SorobanRpc.Server {
    // A blank target avoids Proxy invariants for SDK clients with own read-only properties.
    return new Proxy({} as SorobanRpc.Server, {
      get: (_target, property) => {
        const node =
          this.nodes.find((candidate) => candidate.name === this.active) ||
          this.nodes[0];
        const value: unknown = Reflect.get(node.client, property, node.client);
        if (typeof value !== 'function') return value;
        const method = value as (...args: unknown[]) => Promise<unknown>;
        return async (...args: unknown[]): Promise<unknown> => {
          try {
            return await method.apply(node.client, args);
          } catch (error) {
            node.healthy = false;
            this.recordFailure(node, 'request_failed');
            this.selectActive();
            const backup = this.nodes.find(
              (candidate) => candidate !== node && this.usable(candidate),
            );
            // Submission may have reached the first node; never submit it twice automatically.
            if (!backup || property === 'sendTransaction') throw error;
            const backupValue: unknown = Reflect.get(
              backup.client,
              property,
              backup.client,
            );
            if (typeof backupValue !== 'function') throw error;
            const backupMethod = backupValue as (
              ...args: unknown[]
            ) => Promise<unknown>;
            try {
              return await backupMethod.apply(backup.client, args);
            } catch (backupError) {
              backup.healthy = false;
              this.recordFailure(backup, 'request_failed');
              this.selectActive();
              throw backupError;
            }
          }
        };
      },
    });
  }

  prometheusMetrics(): string {
    const lines = [
      '# HELP soroban_rpc_active Whether an RPC node handles new requests',
      '# TYPE soroban_rpc_active gauge',
      '# HELP soroban_rpc_circuit_state Circuit state: 0 closed, 1 open, 2 half-open',
      '# TYPE soroban_rpc_circuit_state gauge',
      '# HELP soroban_rpc_probe_healthy Whether the last probe succeeded',
      '# TYPE soroban_rpc_probe_healthy gauge',
      '# HELP soroban_rpc_probe_latency_ms Last probe response time in milliseconds',
      '# TYPE soroban_rpc_probe_latency_ms gauge',
      '# HELP soroban_rpc_latest_ledger Latest ledger reported by the RPC node',
      '# TYPE soroban_rpc_latest_ledger gauge',
      '# HELP soroban_rpc_failovers_total Number of active RPC node changes',
      '# TYPE soroban_rpc_failovers_total counter',
    ];
    for (const node of this.nodes) {
      const label = `{node="${node.name}"}`;
      lines.push(
        `soroban_rpc_active${label} ${this.active === node.name ? 1 : 0}`,
      );
      lines.push(
        `soroban_rpc_circuit_state${label} ${{ closed: 0, open: 1, half_open: 2 }[node.state]}`,
      );
      lines.push(`soroban_rpc_probe_healthy${label} ${node.healthy ? 1 : 0}`);
      lines.push(`soroban_rpc_probe_latency_ms${label} ${node.latencyMs}`);
      lines.push(`soroban_rpc_latest_ledger${label} ${node.latestLedger}`);
    }
    lines.push(`soroban_rpc_failovers_total ${this.failovers}`);
    return lines.join('\n') + '\n';
  }
}
