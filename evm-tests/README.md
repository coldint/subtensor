# type-test

The evm-tests folder includes all typescript code to test the basic EVM function
like token transfer, and all precompile contracts in Subtensor. It is
implemented in typescript, use both ethers and viem lib to interact with
contracts. The polkadot API is used to call extrinsic, get storage in Subtensor
. The developers can use it to verify the code change in precompile contracts.

It is also included in the CI process, all test cases are executed for new
commit. CI flow can get catch any failed test cases. The polkadot API get the
latest metadata from the runtime, the case also can find out any incompatibility
between runtime and precompile contracts.

## polkadot api

To get the metadata, you need start the localnet via run
`./scripts/localnet.sh`. then run following command to get metadata, a folder
name .papi will be created, which include the metadata and type definitions.

```bash
npx papi add devnet -w ws://localhost:9944
```

## get the new metadata

If the runtime is upgrade, need to get the metadata again.

```bash
sh get-metadata.sh
```

## run all tests

```bash
yarn run test
```

## To run a particular test case, you can pass an argument with the name or part of the name. For example:

```bash
yarn run test -- -g "Can set subnet parameter"
```
