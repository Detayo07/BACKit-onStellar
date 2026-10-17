import { HttpService } from '@nestjs/axios';
import { of, throwError } from 'rxjs';
import { HealthService } from './health.service';
import { SorobanRpc } from '@stellar/stellar-sdk';

type TestNode = {
  client: SorobanRpc.Server;
  state: string;
  openedAt: number;
};

const getNodes = (service: HealthService): TestNode[] =>
  Reflect.get(service, 'nodes') as TestNode[];

describe('HealthService RPC failover', () => {
  const originalEnv = { ...process.env };
  let post: jest.Mock;
  let service: HealthService;

  const healthy = (ledger: number) =>
    of({
      status: 200,
      data: { result: { status: 'healthy', latestLedger: ledger } },
    });

  beforeEach(() => {
    process.env.STELLAR_RPC_URL = 'https://primary.example';
    process.env.STELLAR_RPC_FALLBACK_URL = 'https://backup.example';
    post = jest.fn(() => healthy(100));
    service = new HealthService({ post } as unknown as HttpService);
  });

  afterAll(() => {
    process.env = originalEnv;
  });

  it('opens the primary circuit after consecutive failures and routes indexing reads to backup', async () => {
    await service.probeAll();
    post.mockImplementation((url: string) =>
      url.includes('primary')
        ? throwError(() => new Error('down'))
        : healthy(100),
    );
    await service.probeAll();
    await service.probeAll();
    await service.probeAll();
    expect(service.getSnapshot().active).toBe('fallback');
    expect(service.getSnapshot().nodes[0].state).toBe('open');

    const nodes = getNodes(service);
    const latestLedger = jest.fn(() => Promise.resolve({ sequence: 100 }));
    Object.defineProperty(nodes[1].client, 'getLatestLedger', {
      value: latestLedger,
      configurable: true,
    });
    await expect(service.getRpcServer().getLatestLedger()).resolves.toEqual({
      sequence: 100,
    });
    expect(latestLedger).toHaveBeenCalledTimes(1);
  });

  it('recovers through a half-open probe after cooldown', async () => {
    post.mockImplementation((url: string) =>
      url.includes('primary')
        ? throwError(() => new Error('down'))
        : healthy(100),
    );
    for (let index = 0; index < 3; index++) await service.probeAll();
    const primary = getNodes(service)[0];
    expect(primary.state).toBe('open');
    primary.openedAt = Date.now() - 30_001;
    post.mockImplementation(() => healthy(101));
    await service.probeAll();
    expect(primary.state).toBe('closed');
    expect(service.getSnapshot().active).toBe('primary');
  });

  it('opens a circuit when intermittent failures exceed 20 percent of five probes', async () => {
    let primaryProbes = 0;
    post.mockImplementation((url: string) => {
      if (url.includes('primary') && [1, 3].includes(++primaryProbes)) {
        return throwError(() => new Error('intermittent failure'));
      }
      return healthy(100);
    });
    for (let index = 0; index < 5; index++) await service.probeAll();
    expect(service.getSnapshot().nodes[0].state).toBe('open');
    expect(service.getSnapshot().active).toBe('fallback');
  });

  it('rejects a lagging node and retries a failed read on backup', async () => {
    post.mockImplementation((url: string) =>
      healthy(url.includes('primary') ? 90 : 100),
    );
    await service.probeAll();
    expect(service.getSnapshot().active).toBe('fallback');
    post.mockImplementation(() => healthy(100));
    await service.probeAll();
    const nodes = getNodes(service);
    Object.defineProperty(nodes[0].client, 'getLatestLedger', {
      value: jest.fn(() => Promise.reject(new Error('connection lost'))),
      configurable: true,
    });
    Object.defineProperty(nodes[1].client, 'getLatestLedger', {
      value: jest.fn(() => Promise.resolve({ sequence: 100 })),
      configurable: true,
    });
    await expect(service.getRpcServer().getLatestLedger()).resolves.toEqual({
      sequence: 100,
    });
    expect(service.getSnapshot().active).toBe('fallback');
  });

  it('exports Prometheus metrics without endpoint URLs', async () => {
    await service.probeAll();
    const metrics = service.prometheusMetrics();
    expect(metrics).toContain('soroban_rpc_latest_ledger{node="primary"} 100');
    expect(metrics).toContain('soroban_rpc_failovers_total 0');
    expect(metrics).not.toContain('primary.example');
  });
});
