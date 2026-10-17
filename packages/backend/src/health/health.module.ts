import { Module } from '@nestjs/common';
import { HttpModule } from '@nestjs/axios';
import { HealthController } from './health.controller';
import { ShutdownService } from './shutdown.service';
import { HealthService } from './health.service';
import { SorobanRpc } from '@stellar/stellar-sdk';

@Module({
  imports: [
    HttpModule, // provides HttpService for Stellar RPC ping
  ],
  controllers: [HealthController],
  providers: [
    ShutdownService,
    HealthService,
    {
      provide: SorobanRpc.Server,
      useFactory: (health: HealthService) => health.getRpcServer(),
      inject: [HealthService],
    },
  ],
  exports: [ShutdownService, HealthService, SorobanRpc.Server],
  // DataSource is provided globally by TypeOrmModule.forRoot() in AppModule
  // so it can be injected directly — no extra registration needed here
})
export class HealthModule {}
