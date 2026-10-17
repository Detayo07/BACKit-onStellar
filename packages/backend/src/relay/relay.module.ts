import { Module } from '@nestjs/common';
import { RelayController } from './relay.controller';
import { RelayService } from './relay.service';
import { PlatformConfigModule } from '../config/config.module';
import { HealthModule } from '../health/health.module';

@Module({
  imports: [PlatformConfigModule, HealthModule],
  controllers: [RelayController],
  providers: [RelayService],
})
export class RelayModule {}
