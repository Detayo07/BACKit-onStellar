import { Module, forwardRef } from '@nestjs/common';
import { TypeOrmModule } from '@nestjs/typeorm';
import { OracleService } from './oracle.service';
import { OracleController } from './oracle.controller';
import { PriceFetcherService } from './price-fetcher.service';
import { SigningService } from './signing.service';
import { OracleCall } from './entities/oracle-call.entity';
import { OracleOutcome } from './entities/oracle-outcome.entity';
import { CallsModule } from '../calls/calls.module';
import { CoinGeckoService } from './coinGeko.service';
import { PriceDeviationService } from './deiviation.service';
import { PriceDeviationWorker } from './deviation.worker';
import { PriceDeviationLog } from './entities/log.entity';
import { OraclePriceEntity } from './entities/storedOraclePrice.entity';
import { OracleHealthLog } from './entities/oracle-health-log.entity';
import { OracleHealthService } from './oracle-health.service';
import { OracleHealthController } from './oracle-health.controller';
import { IpfsService } from '../storage/ipfs.service';
import { Token } from '../token/entities/token.entity';
import { RpcFailoverModule } from '../health/rpc-failover.module';

@Module({
  imports: [
    TypeOrmModule.forFeature([
      OracleCall,
      OracleOutcome,
      PriceDeviationLog,
      OraclePriceEntity,
      OracleHealthLog,
      Token,
    ]),
    forwardRef(() => CallsModule),
    RpcFailoverModule,
  ],
  controllers: [OracleController, OracleHealthController],
  providers: [
    OracleService,
    PriceFetcherService,
    SigningService,
    IpfsService,
    OracleHealthService,
    CoinGeckoService,
    PriceDeviationService,
    PriceDeviationWorker,
  ],
  exports: [
    OracleService,
    PriceDeviationService,
    OracleHealthService,
    CoinGeckoService,
  ],
})
export class OracleModule {}
