import { Module } from '@nestjs/common';
import { RelayController } from './relay.controller';
import { RelayService } from './relay.service';
import { PlatformConfigModule } from '../config/config.module';
import { RpcFailoverModule } from '../health/rpc-failover.module';

@Module({
  imports: [PlatformConfigModule, RpcFailoverModule],
  controllers: [RelayController],
  providers: [RelayService],
})
export class RelayModule {}
