import { Module } from '@nestjs/common';
import { TypeOrmModule } from '@nestjs/typeorm';
import { BullModule } from '@nestjs/bullmq';
import { ReconciliationRun } from './entities/reconciliation-run.entity';
import { ReconciliationDiscrepancy } from './entities/reconciliation-discrepancy.entity';
import { Call } from '../calls/entities/call.entity';
import { Stake } from '../stakes/entities/stake.entity';
import { PayoutClaim } from '../payouts/entities/payout-claim.entity';
import { EventLog } from '../indexer/event-log.entity';
import { EventStoreEntry } from '../event-store/entities/event-store-entry.entity';
import { ReconciliationService } from './reconciliation.service';
import { ReconciliationController } from './reconciliation.controller';
import { ReconciliationProcessor } from './reconciliation.processor';
import { QUEUE_RECONCILIATION } from '../common/queues/queues.constants';
import { AuthModule } from '../auth/auth.module';
import { RpcFailoverModule } from '../health/rpc-failover.module';

@Module({
  imports: [
    TypeOrmModule.forFeature([
      ReconciliationRun,
      ReconciliationDiscrepancy,
      Call,
      Stake,
      PayoutClaim,
      EventLog,
      EventStoreEntry,
    ]),
    BullModule.registerQueue({
      name: QUEUE_RECONCILIATION,
    }),
    AuthModule,
    RpcFailoverModule,
  ],
  controllers: [ReconciliationController],
  providers: [ReconciliationService, ReconciliationProcessor],
  exports: [ReconciliationService],
})
export class ReconciliationModule {}
