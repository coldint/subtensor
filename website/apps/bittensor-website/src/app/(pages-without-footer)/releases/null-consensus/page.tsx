import FadeInWrapper from '@/app/components/FadeInWrapper';
import {Link} from '@raofoundation/ui';
import type {Metadata} from 'next';
import {Suspense} from 'react';
import styles from '../v436-upgrade/page.module.css';

export const metadata: Metadata = {
  title: 'Null Consensus — Large Subnets and Exact Weight Ratios',
  description:
    'Opt-in Null consensus selects the highest-stake validator for miner rewards, pays ' +
    'stake-proportional dividends, and supports a shared capacity of 16,000 UIDs.',
  alternates: {canonical: '/releases/null-consensus'},
};

export default function Page() {
  return (
    <Suspense fallback={<div style={{minHeight: '100vh', backgroundColor: 'white'}} />}>
      <FadeInWrapper className={styles.page_container}>
        <section className={styles.title_section}>
          <h1 className={styles.paper_title}>Null Consensus</h1>
          <p className={styles.subtitle}>Large subnets and exact weight ratios · Upcoming</p>
        </section>
        <section className={styles.section}>
          <h2 className={styles.subtitle}>What changes</h2>
          <p>
            Subnet owners can opt into a second epoch mechanism. Null consensus takes miner
            incentives from the permitted validator with the highest eligible stake. Ties select the
            first UID. Other validators retain stake-proportional dividends, including inactive
            validators, while their weights do not affect incentives. When the selected row has no
            valid weights, every registered UID shares the miner budget equally. Yuma remains the
            default for existing and new subnets.
          </p>
          <p>
            Null skips consensus and bond calculations and leaves bond storage unchanged. The epoch
            and payout interfaces remain compatible. Existing stake thresholds, permits,
            stale-destination checks and commit-reveal protections still apply.
          </p>
        </section>
        <section className={styles.section}>
          <h2 className={styles.subtitle}>Capacity and switching</h2>
          <p>
            Null shares a 16,000-UID capacity budget across emission mechanisms: 16,000 with one
            mechanism, 8,000 with two, and 4,000 with four. Switching to Null preserves current
            capacity until the owner raises it. Returning to Yuma requires the registered population
            to fit its shared 256-UID budget; successful switching also clamps configured capacity.
            Pruning is an explicit owner action.
          </p>
        </section>
        <section className={styles.section}>
          <h2 className={styles.subtitle}>Precise rewards and large weight submissions</h2>
          <p>
            Null stores submitted unsigned 16-bit weights without max-scaling. The SDK and CLI
            provide an explicit raw-integer path, including JSON file input for large rows. Exact
            wide integer arithmetic computes rewards before display values are quantized. Whole-unit
            remainders are allocated deterministically in UID order; total miner emissions never
            exceed their epoch budget. Fractional entitlements are not carried to later epochs.
          </p>
          <p>
            Null timelocked ciphertexts can be up to 128 KiB, leaving room above a full 16,000-entry
            row. Queues share byte and count limits across mechanisms to bound storage and reveal
            work. Yuma retains its 5,000-byte admission limit.
          </p>
          <pre className={styles.code_block}>{`# Subnet owner: switch, then raise capacity
btcli hparams set --netuid 1 --name epoch_consensus --value Null
btcli hparams set --netuid 1 --name max_allowed_uids --value 16000

# Validator: weights.json contains {"2": 65534, "3": 1, ...}
btcli misc weights set --netuid 1 --raw-u16 --weights-file weights.json`}</pre>
          <p>
            See the <Link href='/docs/guides/null-consensus'>Null consensus guide</Link> for
            validation rules, integer rounding and returning to Yuma.
          </p>
        </section>
      </FadeInWrapper>
    </Suspense>
  );
}
