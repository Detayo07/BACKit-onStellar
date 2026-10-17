import { HttpModule } from '@nestjs/axios';
import { Module } from '@nestjs/common';
import { SorobanRpc } from '@stellar/stellar-sdk';
import { HealthService } from './health.service';

/** RPC routing has no database dependency, so feature modules can import it alone. */
@Module({
  imports: [HttpModule],
  providers: [
    HealthService,
    {
      provide: SorobanRpc.Server,
      useFactory: (health: HealthService) => health.getRpcServer(),
      inject: [HealthService],
    },
  ],
  exports: [HealthService, SorobanRpc.Server],
})
export class RpcFailoverModule {}
