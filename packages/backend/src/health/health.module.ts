import { Module } from '@nestjs/common';
import { HealthController } from './health.controller';
import { ShutdownService } from './shutdown.service';
import { RpcFailoverModule } from './rpc-failover.module';

@Module({
  imports: [RpcFailoverModule],
  controllers: [HealthController],
  providers: [ShutdownService],
  exports: [ShutdownService],
})
export class HealthModule {}
