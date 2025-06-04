export const IUID_LOOKUP_ADDRESS = "0x0000000000000000000000000000000000000806";

export const IUIDLookupABI = [
    {
        inputs: [
            {
                internalType: "uint16",
                name: "netuid",
                type: "uint16"
            },
            {
                internalType: "address",
                name: "evm_address",
                type: "address"
            },
            {
                internalType: "uint16",
                name: "limit",
                type: "uint16"
            }
        ],
        name: "uidLookup",
        outputs: [
            {
                components: [
                    {
                        internalType: "uint16",
                        name: "uid",
                        type: "uint16"
                    },
                    {
                        internalType: "uint64",
                        name: "block_associated",
                        type: "uint64"
                    }
                ],
                internalType: "struct LookupItem[]",
                name: "",
                type: "tuple[]"
            }
        ],
        stateMutability: "view",
        type: "function"
    }
];
