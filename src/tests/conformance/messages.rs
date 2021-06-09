use crate::{
    helpers::{
        initiate_handshake,
        synthetic_peers::{SyntheticNode, SyntheticNodeConfig},
        TIMEOUT,
    },
    protocol::{
        message::{
            filter::{Filter, MessageFilter},
            Message,
        },
        payload::{
            addr::NetworkAddr,
            block::{Block, Headers, LocatorHashes},
            inv::{InvHash, ObjectKind},
            reject::CCode,
            Addr, FilterAdd, FilterLoad, Hash, Inv, Nonce, Version,
        },
    },
    setup::{
        config::new_local_addr,
        node::{Action, Node},
    },
    wait_until,
};

use assert_matches::assert_matches;
use tokio::time::timeout;

use std::{net::SocketAddr, time::Duration};

#[tokio::test]
async fn ping_pong() {
    // Create a synthetic node and enable handshaking.
    let mut synthetic_node = SyntheticNode::new(SyntheticNodeConfig {
        enable_handshaking: true,
        ..Default::default()
    })
    .await
    .unwrap();

    // Spin up a node.
    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection(new_local_addr()))
        .start()
        .await;

    // Connect to the node and handshake.
    synthetic_node.connect(node.addr()).await.unwrap();

    // Send ping.
    let ping_nonce = Nonce::default();
    synthetic_node
        .send_direct_message(node.addr(), Message::Ping(ping_nonce))
        .await
        .unwrap();

    // Recieve pong and verify the nonce matches.
    let (_, pong) = synthetic_node.recv_message().await;
    assert_matches!(pong, Message::Pong(pong_nonce) if pong_nonce == ping_nonce);

    node.stop().await;
}

#[tokio::test]
async fn reject_invalid_messages() {
    // ZG-CONFORMANCE-008
    //
    // The node rejects handshake and bloom filter messages post-handshake.
    //
    // The following messages should be rejected post-handshake:
    //
    //     Version     (Duplicate)
    //     Verack      (Duplicate)
    //     Inv         (Invalid -- with mixed types)
    //     FilterLoad  (Obsolete)
    //     FilterAdd   (Obsolete)
    //     FilterClear (Obsolete)
    //
    //     Inv (tbd)   (Invalid -- with multiple advertised blocks, for zebra this is not an error)
    //
    // Test procedure (for each message):
    //
    //      1. Connect and complete the handshake
    //      2. Send the test message
    //      3. Filter out all node queries
    //      4. Receive `Reject(kind)`
    //      5. Assert that `kind` is appropriate for the test message
    //
    // zebra: doesn't send reject and terminates the connection.
    // zcashd:
    //
    //     Version:            passes
    //     Verack:             ignored
    //     Mixed Inv:          ignored
    //     Multi-Block Inv:    ignored
    //     FilterLoad:         Reject(Malformed) - needs investigation
    //     FilterAdd:          Reject(Malformed) - needs investigation
    //     FilterClear:        ignored
    //     FilterClear:        ignored

    // Spin up a node instance (so we have acces to its addr).
    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection(new_local_addr()))
        .start()
        .await;

    // Generate a mixed Inventory hash set.
    let genesis_block = Block::testnet_genesis();
    let mixed_inv = vec![genesis_block.inv_hash(), genesis_block.txs[0].inv_hash()];
    let multi_block_inv = vec![
        genesis_block.inv_hash(),
        genesis_block.inv_hash(),
        genesis_block.inv_hash(),
    ];

    // List of test messages and their expected Reject kind.
    let cases = vec![
        (
            Message::Version(Version::new(node.addr(), new_local_addr())),
            CCode::Duplicate,
        ),
        (Message::Verack, CCode::Duplicate),
        (Message::Inv(Inv::new(mixed_inv)), CCode::Invalid),
        (Message::Inv(Inv::new(multi_block_inv)), CCode::Invalid),
        (Message::FilterLoad(FilterLoad::default()), CCode::Obsolete),
        (Message::FilterAdd(FilterAdd::default()), CCode::Obsolete),
        (Message::FilterClear, CCode::Obsolete),
    ];

    // Configuration for all the synthetic nodes.
    let config = SyntheticNodeConfig {
        enable_handshaking: true,
        message_filter: MessageFilter::with_all_enabled(),
        ..Default::default()
    };

    for (test_message, expected_ccode) in cases {
        // Start a synthetic node.
        let mut synthetic_node = SyntheticNode::new(config.clone()).await.unwrap();

        // Connect and initiate handshake.
        synthetic_node.connect(node.addr()).await.unwrap();

        // Send the test message.
        synthetic_node
            .send_direct_message(node.addr(), test_message.clone())
            .await
            .unwrap();

        // Expect a Reject(Invalid) message.
        let (_, message) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
        assert_matches!(message, Message::Reject(reject) if reject.ccode == expected_ccode);

        // Gracefully shut down the synthetic node.
        synthetic_node.shut_down();
    }

    // Gracefully shut down the node.
    node.stop().await;
}

#[tokio::test]
async fn ignores_unsolicited_responses() {
    // ZG-CONFORMANCE-009
    //
    // The node ignore certain unsolicited messages but doesn’t disconnect.
    //
    // Messages to be tested: Reject, NotFound, Pong, Tx, Block, Header, Addr.
    //
    // Test procedure:
    //      Complete handshake, and then for each test message:
    //
    //      1. Send the message
    //      2. Send a ping request
    //      3. Receive a pong response

    // Spin up a node instance.
    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection(new_local_addr()))
        .start()
        .await;

    // Create a synthetic node.
    let mut synthetic_node = SyntheticNode::new(SyntheticNodeConfig {
        enable_handshaking: true,
        message_filter: MessageFilter::with_all_enabled(),
        ..Default::default()
    })
    .await
    .unwrap();

    // Connect and initiate the handshake.
    synthetic_node.connect(node.addr()).await.unwrap();

    let test_messages = vec![
        Message::Pong(Nonce::default()),
        Message::Headers(Headers::empty()),
        Message::Addr(Addr::empty()),
        Message::Block(Box::new(Block::testnet_genesis())),
        Message::NotFound(Inv::new(vec![Block::testnet_1().txs[0].inv_hash()])),
        Message::Tx(Block::testnet_2().txs[0].clone()),
    ];

    for message in test_messages {
        // Send the unsolicited message.
        synthetic_node
            .send_direct_message(node.addr(), message)
            .await
            .unwrap();

        // A response to ping would indicate the previous message was ignored.
        synthetic_node.assert_ping_pong(node.addr()).await;
    }

    // Gracefully shut down the nodes.
    synthetic_node.shut_down();
    node.stop().await;
}

#[tokio::test]
async fn basic_query_response_seeded() {
    // ZG-CONFORMANCE-010, node is seeded with data
    //
    // The node responds with the correct messages. Message correctness is naively verified through successful encoding/decoding.
    //
    // `Ping` expects `Pong`.
    // `GetAddr` expects `Addr`.
    // `Mempool` expects `Inv`.
    // `Getblocks` expects `Inv`.
    // `GetData(tx_hash)` expects `Tx`.
    // `GetData(block_hash)` expects `Blocks`.
    // `GetHeaders` expects `Headers`.
    //
    // zebra: DoS `GetData` spam due to auto-response
    // zcashd: ignores the following messages
    //             - GetAddr
    //             - MemPool
    //             - GetBlocks
    //
    //         GetData(tx) returns NotFound (which is correct),
    //         because we currently can't seed a mempool.
    //

    let genesis_block = Block::testnet_genesis();

    // Spin up a node instance.
    let mut node: Node = Default::default();
    node.initial_action(Action::SeedWithTestnetBlocks {
        socket_addr: new_local_addr(),
        block_count: 3,
    })
    .start()
    .await;

    // Create a synthetic node.
    let mut synthetic_node = SyntheticNode::new(SyntheticNodeConfig {
        enable_handshaking: true,
        message_filter: MessageFilter::with_all_auto_reply(),
        ..Default::default()
    })
    .await
    .unwrap();

    // Connect to the node and initiate handshake.
    synthetic_node.connect(node.addr()).await.unwrap();

    // Ping/Pong.
    {
        let ping_nonce = Nonce::default();
        synthetic_node
            .send_direct_message(node.addr(), Message::Ping(ping_nonce))
            .await
            .unwrap();

        // Verify the nonce matches.
        let (_, pong) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
        assert_matches!(pong, Message::Pong(pong_nonce) if pong_nonce == ping_nonce);
    }

    // GetAddr/Addr.
    {
        synthetic_node
            .send_direct_message(node.addr(), Message::GetAddr)
            .await
            .unwrap();

        let (_, addr) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
        assert_matches!(addr, Message::Addr(..));
    }

    // MemPool/Inv.
    {
        synthetic_node
            .send_direct_message(node.addr(), Message::MemPool)
            .await
            .unwrap();

        let (_, inv) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
        assert_matches!(inv, Message::Inv(..));
    }

    // GetBlocks/Inv (requesting testnet genesis).
    {
        synthetic_node
            .send_direct_message(
                node.addr(),
                Message::GetBlocks(LocatorHashes::new(
                    vec![genesis_block.double_sha256().unwrap()],
                    Hash::zeroed(),
                )),
            )
            .await
            .unwrap();

        let (_, inv) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
        assert_matches!(inv, Message::Inv(..));
    }

    // GetData/Tx.
    {
        synthetic_node
            .send_direct_message(
                node.addr(),
                Message::GetData(Inv::new(vec![genesis_block.txs[0].inv_hash()])),
            )
            .await
            .unwrap();

        let (_, tx) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
        assert_matches!(tx, Message::Tx(..));
    }

    // GetData/Block.
    {
        synthetic_node
            .send_direct_message(
                node.addr(),
                Message::GetData(Inv::new(vec![Block::testnet_2().inv_hash()])),
            )
            .await
            .unwrap();

        let (_, block) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
        assert_matches!(block, Message::Block(..));
    }

    // GetHeaders/Headers.
    {
        synthetic_node
            .send_direct_message(
                node.addr(),
                Message::GetHeaders(LocatorHashes::new(
                    vec![genesis_block.double_sha256().unwrap()],
                    Hash::zeroed(),
                )),
            )
            .await
            .unwrap();

        let (_, headers) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
        assert_matches!(headers, Message::Headers(..));
    }

    // Gracefully shut down the nodes.
    synthetic_node.shut_down();
    node.stop().await;
}

#[tokio::test]
async fn basic_query_response_unseeded() {
    // ZG-CONFORMANCE-010, node is *not* seeded with data
    //
    // The node responds with the correct messages. Message correctness is naively verified through successful encoding/decoding.
    //
    // `GetData(tx_hash)` expects `NotFound`.
    // `GetData(block_hash)` expects `NotFound`.
    //
    // The test currently fails for zcashd and zebra
    //
    // Current behaviour:
    //
    //  zebra: DDoS spam due to auto-response
    //  zcashd: Ignores `GetData(block_hash)`

    // GetData messages...
    let messages = vec![
        // ...with a tx hash...
        Message::GetData(Inv::new(vec![Block::testnet_genesis().txs[0].inv_hash()])),
        // ...and with a block hash.
        Message::GetData(Inv::new(vec![Block::testnet_2().inv_hash()])),
    ];

    // Spin up a node instance.
    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection(new_local_addr()))
        .start()
        .await;

    // Create a synthetic node with message filtering.
    let mut synthetic_node = SyntheticNode::new(SyntheticNodeConfig {
        enable_handshaking: true,
        message_filter: MessageFilter::with_all_enabled(),
        ..Default::default()
    })
    .await
    .unwrap();

    // Connect to the node and initiate the handshake.
    synthetic_node.connect(node.addr()).await.unwrap();

    for message in messages {
        // Send GetData.
        synthetic_node
            .send_direct_message(node.addr(), message)
            .await
            .unwrap();

        // Assert NotFound is returned.
        // FIXME: assert on hash?
        let (_, reply) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
        assert_matches!(reply, Message::NotFound(..));
    }

    // Gracefully shut down the nodes.
    synthetic_node.shut_down();
    node.stop().await;
}

#[tokio::test]
async fn disconnects_for_trivial_issues() {
    // ZG-CONFORMANCE-011
    //
    // The node disconnects for trivial (non-fuzz, non-malicious) cases.
    //
    // - `Ping` timeout (not tested due to 20minute zcashd timeout).
    // - `Pong` with wrong nonce.
    // - `GetData` with mixed types in inventory list.
    // - `Inv` with mixed types in inventory list.
    // - `Addr` with `NetworkAddr` with no timestamp.
    //
    // Note: Ping with timeout test case is not exercised as the zcashd timeout is
    //       set to 20 minutes, which is simply too long.
    //
    // Note: Addr test requires commenting out the relevant code in the encode
    //       function of NetworkAddr as we cannot encode without a timestamp.
    //
    // This test currently fails for zcashd and zebra.
    //
    // Current behaviour:
    //
    //  zcashd:
    //      GetData(mixed)  - responds to both
    //      Inv(mixed)      - ignores the message
    //      Addr            - Reject(Malformed), but no DC
    //
    //  zebra:
    //      Pong            - ignores the message

    // Spin up a node instance.
    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection(new_local_addr()))
        .start()
        .await;

    // Configuration letting through ping messages for the first case.
    let config = SyntheticNodeConfig {
        enable_handshaking: true,
        message_filter: MessageFilter::with_all_auto_reply().with_ping_filter(Filter::Disabled),
        ..Default::default()
    };

    // Pong with bad nonce.
    {
        let mut synthetic_node = SyntheticNode::new(config.clone()).await.unwrap();
        synthetic_node.connect(node.addr()).await.unwrap();

        match synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap() {
            (_, Message::Ping(_)) => synthetic_node
                .send_direct_message(node.addr(), Message::Pong(Nonce::default()))
                .await
                .unwrap(),

            message => panic!("Unexpected message while waiting for Ping: {:?}", message),
        }

        wait_until!(TIMEOUT, synthetic_node.num_connected() == 0);
        synthetic_node.shut_down();
    }

    // Update the filter to include ping messages.
    let config = SyntheticNodeConfig {
        message_filter: MessageFilter::with_all_auto_reply(),
        ..config
    };

    // GetData with mixed inventory.
    {
        let synthetic_node = SyntheticNode::new(config.clone()).await.unwrap();
        synthetic_node.connect(node.addr()).await.unwrap();

        let genesis_block = Block::testnet_genesis();
        let mixed_inv = vec![genesis_block.inv_hash(), genesis_block.txs[0].inv_hash()];

        synthetic_node
            .send_direct_message(node.addr(), Message::GetData(Inv::new(mixed_inv.clone())))
            .await
            .unwrap();

        wait_until!(TIMEOUT, synthetic_node.num_connected() == 0);
        synthetic_node.shut_down();
    }

    // Inv with mixed inventory (using non-genesis block since all node's "should" have genesis already,
    // which makes advertising it non-sensical).
    {
        let synthetic_node = SyntheticNode::new(config).await.unwrap();
        synthetic_node.connect(node.addr()).await.unwrap();

        let block_1 = Block::testnet_1();
        let mixed_inv = vec![block_1.inv_hash(), block_1.txs[0].inv_hash()];

        synthetic_node
            .send_direct_message(node.addr(), Message::Inv(Inv::new(mixed_inv)))
            .await
            .unwrap();

        wait_until!(TIMEOUT, synthetic_node.num_connected() == 0);
        synthetic_node.shut_down();
    }

    // Gracefully shut down the node.
    node.stop().await;
}

#[tokio::test]
async fn eagerly_crawls_network_for_peers() {
    // ZG-CONFORMANCE-012
    //
    // The node crawls the network for new peers and eagerly connects.
    //
    // Test procedure:
    //
    //  1. Create a set of peer nodes, listening concurrently
    //  2. Connect to node with another main peer node
    //  3. Wait for `GetAddr`
    //  4. Send set of peer listener node addresses
    //  5. Expect the node to connect to each peer in the set
    //
    // zcashd: Has different behaviour depending on connection direction.
    //         If we initiate the main connection it sends Ping, GetHeaders,
    //         but never GetAddr.
    //         If the node initiates then it does send GetAddr, but it never connects
    //         to the peers.
    //
    // zebra:  Fails, unless we keep responding on the main connection.
    //         If we do not keep responding then the peer connections take really long to establish,
    //         failing the test completely.
    //
    //         Related issues: https://github.com/ZcashFoundation/zebra/pull/2154
    //                         https://github.com/ZcashFoundation/zebra/issues/2163

    // Spin up a node instance.
    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection(new_local_addr()))
        .start()
        .await;

    // Create 5 synthetic nodes.
    const N: usize = 5;
    let mut synthetic_nodes = Vec::with_capacity(N);
    for _ in 0..N {
        let synthetic_node = SyntheticNode::new(SyntheticNodeConfig {
            enable_handshaking: true,
            message_filter: MessageFilter::with_all_auto_reply(),
            ..Default::default()
        })
        .await
        .unwrap();

        synthetic_nodes.push(synthetic_node);
    }

    // Collect their addrs.
    let addrs = synthetic_nodes
        .iter()
        .map(|node| node.listening_addr())
        .map(|addr| NetworkAddr::new(addr))
        .collect::<Vec<_>>();

    // Adjust the config so it lets through GetAddr message and start a "main" synthetic node which
    // will provide the peer list.
    let mut synthetic_node = SyntheticNode::new(SyntheticNodeConfig {
        enable_handshaking: true,
        message_filter: MessageFilter::with_all_auto_reply().with_getaddr_filter(Filter::Disabled),
        ..Default::default()
    })
    .await
    .unwrap();

    // Connect and handshake.
    synthetic_node.connect(node.addr()).await.unwrap();

    // Expect GetAddr.
    let (_, getaddr) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
    assert_matches!(getaddr, Message::GetAddr);

    // Respond with peer list.
    synthetic_node
        .send_direct_message(node.addr(), Message::Addr(Addr::new(addrs)))
        .await
        .unwrap();

    // Expect the synthetic nodes to get a connection request from the node.
    for node in synthetic_nodes {
        wait_until!(TIMEOUT, node.num_connected() == 1);
        node.shut_down();
    }

    // Gracefully shut down the node.
    node.stop().await;
}

#[tokio::test]
async fn correctly_lists_peers() {
    // ZG-CONFORMANCE-013
    //
    // The node responds to a `GetAddr` with a list of peers it’s connected to. This command
    // should only be sent once, and by the node initiating the connection.
    //
    // In addition, this test case exercises the known zebra bug: https://github.com/ZcashFoundation/zebra/pull/2120
    //
    // Test procedure
    //      1. Establish N peer listeners
    //      2. Start node which connects to these N peers
    //      3. Create i..M new connections which,
    //          a) Connect to the node
    //          b) Query GetAddr
    //          c) Receive Addr == N peer addresses
    //
    // This test currently fails for both zcashd and zebra.
    //
    // Current behaviour:
    //
    //  zcashd: Never responds. Logs indicate `Unknown command "getaddr" from peer=1` if we initiate
    //          the connection. If the node initiates the connection then the command is recoginized,
    //          but likely ignored (because only the initiating node is supposed to send it).
    //
    //  zebra:  Never responds: "zebrad::components::inbound: ignoring `Peers` request from remote peer during network setup"
    //
    //          Can be coaxed into responding by sending a non-empty Addr in
    //          response to node's GetAddr. This still fails as it includes previous inbound
    //          connections in its address book (as in the bug listed above).

    // Create 5 synthetic nodes.
    const N: usize = 5;
    let mut synthetic_nodes = Vec::with_capacity(N);
    for _ in 0..N {
        let synthetic_node = SyntheticNode::new(SyntheticNodeConfig {
            enable_handshaking: true,
            message_filter: MessageFilter::with_all_auto_reply(),
            ..Default::default()
        })
        .await
        .unwrap();

        synthetic_nodes.push(synthetic_node);
    }

    // Collect their addrs.
    let expected_addrs: Vec<SocketAddr> = synthetic_nodes
        .iter()
        .map(|node| node.listening_addr())
        .collect();

    // Start node with the synthetic nodes as initial peers.
    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection(new_local_addr()))
        .initial_peers(expected_addrs.clone())
        .start()
        .await;

    // Connect to node and request GetAddr. We perform multiple iterations to exercise the #2120
    // zebra bug.
    for _ in 0..N {
        let mut synthetic_node = SyntheticNode::new(SyntheticNodeConfig {
            enable_handshaking: true,
            message_filter: MessageFilter::with_all_auto_reply(),
            ..Default::default()
        })
        .await
        .unwrap();

        synthetic_node.connect(node.addr()).await.unwrap();
        synthetic_node
            .send_direct_message(node.addr(), Message::GetAddr)
            .await
            .unwrap();

        let (_, addr) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
        let addrs = assert_matches!(addr, Message::Addr(addrs) => addrs);

        // Check that ephemeral connections were not gossiped.
        let addrs: Vec<SocketAddr> = addrs.iter().map(|network_addr| network_addr.addr).collect();
        assert_eq!(addrs, expected_addrs);

        synthetic_node.shut_down();
    }

    // Gracefully shut down nodes.
    for synthetic_node in synthetic_nodes {
        synthetic_node.shut_down();
    }

    node.stop().await;
}

#[tokio::test]
async fn get_blocks() {
    // ZG-CONFORMANCE-015
    //
    // The node responds to `GetBlocks` requests with a list of blocks based on the provided range.
    //
    // We test the following conditions:
    //  1. unlimited queries i.e. stop_hash = 0
    //  2. range queries i.e. stop_hash = i
    //  3. a forked chain (we submit a valid hash, followed by incorrect hashes)
    //
    // Test procedure:
    //  1. Create a node and seed it with the testnet chain
    //  2. Establish a peer node
    //  3. For each test case:
    //      a) send GetBlocks
    //      b) receive Inv
    //      c) assert Inv received matches expectations
    //
    // The test currently fails for both Zebra and zcashd.
    //
    // Current behaviour:
    //
    //  zcashd: Passes
    //
    //  zebra: does not support seeding as yet, and therefore cannot perform this test.
    //
    // Note: zcashd excludes the `stop_hash` from the range, whereas the spec states that it should be inclusive.
    //       We are taking current behaviour as correct.
    //
    // Note: zcashd ignores requests for the final block in the chain

    // Create a node with knowledge of the initial three testnet blocks
    let mut node: Node = Default::default();
    node.initial_action(Action::SeedWithTestnetBlocks {
        socket_addr: new_local_addr(),
        block_count: 3,
    })
    .start()
    .await;

    let blocks = Block::initial_testnet_blocks();

    let mut synthetic_node = SyntheticNode::new(SyntheticNodeConfig {
        enable_handshaking: true,
        message_filter: MessageFilter::with_all_auto_reply(),
        ..Default::default()
    })
    .await
    .unwrap();

    synthetic_node.connect(node.addr()).await.unwrap();

    // Test unlimited range queries, where given the hash for block i we expect all
    // of its children as a reply. This does not apply for the last block in the chain,
    // so we skip it.
    //
    // i.e. Test that GetBlocks(i) -> Inv(i+1..)
    for (i, block) in blocks.iter().enumerate().take(2) {
        synthetic_node
            .send_direct_message(
                node.addr(),
                Message::GetBlocks(LocatorHashes::new(
                    vec![block.double_sha256().unwrap()],
                    Hash::zeroed(),
                )),
            )
            .await
            .unwrap();

        let (_, inv) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
        let inv = assert_matches!(inv, Message::Inv(inv) => inv);

        // Collect inventory hashes for all blocks after i (i's children) and check the payload
        // matches.
        let inv_hashes = blocks.iter().skip(i + 1).map(|b| b.inv_hash()).collect();
        let expected = Inv::new(inv_hashes);
        assert_eq!(inv, expected);
    }

    // Test that we get no response for the final block in the known-chain
    // (this is the behaviour exhibited by zcashd - a more well-formed response
    // might be sending an empty inventory instead).
    synthetic_node
        .send_direct_message(
            node.addr(),
            Message::GetBlocks(LocatorHashes::new(
                vec![blocks.last().unwrap().double_sha256().unwrap()],
                Hash::zeroed(),
            )),
        )
        .await
        .unwrap();

    // Test message is ignored by sending Ping and receiving Pong.
    synthetic_node.assert_ping_pong(node.addr()).await;

    // Test `hash_stop` (it should be included in the range, but zcashd excludes it -- see note).
    synthetic_node
        .send_direct_message(
            node.addr(),
            Message::GetBlocks(LocatorHashes::new(
                vec![blocks[0].double_sha256().unwrap()],
                blocks[2].double_sha256().unwrap(),
            )),
        )
        .await
        .unwrap();

    let (_, inv) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
    let inv = assert_matches!(inv, Message::Inv(inv) => inv);

    // Check the payload matches.
    let expected = Inv::new(vec![blocks[1].inv_hash()]);
    assert_eq!(inv, expected);

    // Test that we get corrected if we are "off chain".
    // We expect that unknown hashes get ignored, until it finds a known hash; it then returns
    // all known children of that block.
    let locators = LocatorHashes::new(
        vec![
            blocks[1].double_sha256().unwrap(),
            Hash::new([19; 32]),
            Hash::new([22; 32]),
        ],
        Hash::zeroed(),
    );

    synthetic_node
        .send_direct_message(node.addr(), Message::GetBlocks(locators))
        .await
        .unwrap();

    let (_, inv) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
    let inv = assert_matches!(inv, Message::Inv(inv) => inv);

    // Check the payload matches.
    let expected = Inv::new(vec![blocks[2].inv_hash()]);
    assert_eq!(inv, expected);

    synthetic_node.shut_down();
    node.stop().await;
}

#[tokio::test]
async fn correctly_lists_blocks() {
    // ZG-CONFORMANCE-016
    //
    // The node responds to `GetHeaders` request with a list of block headers based on the provided range.
    //
    // We test the following conditions:
    //  1. unlimited queries i.e. stop_hash = 0
    //  2. range queries i.e. stop_hash = i
    //  3. a forked chain (we submit a header which doesn't match the chain)
    //
    // Test procedure:
    //  1. Create a node and seed it with the testnet chain
    //  2. Establish a peer node
    //  3. For each test case:
    //      a) send GetHeaders
    //      b) receive Headers
    //      c) assert headers received match expectations
    //
    // The test currently fails for both Zebra and zcashd.
    //
    // Current behaviour:
    //
    //  zcashd: Fails for range queries where the head of the chain equals the stop hash. We expect to receive an empty set,
    //          but instead we get header [i+1] (which exceeds stop_hash).
    //
    //  zebra: does not support seeding as yet, and therefore cannot perform this test.

    // Create a node with knowledge of the initial three testnet blocks
    let mut node: Node = Default::default();
    node.initial_action(Action::SeedWithTestnetBlocks {
        socket_addr: new_local_addr(),
        block_count: 3,
    })
    .start()
    .await;

    // block headers and hashes
    let expected = Block::initial_testnet_blocks()
        .iter()
        .map(|block| block.header.clone())
        .collect::<Vec<_>>();
    let hashes = expected
        .iter()
        .map(|header| header.double_sha256().unwrap())
        .collect::<Vec<_>>();

    // locator hashes are stored in reverse order
    let locator = vec![
        vec![hashes[0]],
        vec![hashes[1], hashes[0]],
        vec![hashes[2], hashes[1], hashes[0]],
    ];

    // Establish a peer node
    let mut stream = initiate_handshake(node.addr()).await.unwrap();
    let filter = MessageFilter::with_all_auto_reply();

    // Query for all blocks from i onwards (stop_hash = [0])
    for i in 0..expected.len() {
        Message::GetHeaders(LocatorHashes::new(locator[i].clone(), Hash::zeroed()))
            .write_to_stream(&mut stream)
            .await
            .unwrap();

        match filter.read_from_stream(&mut stream).await.unwrap() {
            Message::Headers(headers) => assert_eq!(
                headers.headers,
                expected[(i + 1)..],
                "test for Headers([{}..])",
                i
            ),
            messsage => panic!("Expected Headers, but got: {:?}", messsage),
        }
    }

    // Query for all possible valid ranges
    let ranges: Vec<(usize, usize)> = vec![(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 2)];
    for (start, stop) in ranges {
        Message::GetHeaders(LocatorHashes::new(locator[start].clone(), hashes[stop]))
            .write_to_stream(&mut stream)
            .await
            .unwrap();

        // We use start+1 because Headers should list the blocks starting *after* the
        // final location in GetHeaders, and up (and including) the stop-hash.
        match filter.read_from_stream(&mut stream).await.unwrap() {
            Message::Headers(headers) => assert_eq!(
                headers.headers,
                expected[start + 1..=stop],
                "test for Headers([{}..={}])",
                start + 1,
                stop
            ),
            messsage => panic!("Expected Headers, but got: {:?}", messsage),
        }
    }

    // Query as if from a fork. We replace [2], and expect to be corrected
    let mut fork_locator = locator[1].clone();
    fork_locator.insert(0, Hash::new([17; 32]));
    Message::GetHeaders(LocatorHashes::new(fork_locator, Hash::zeroed()))
        .write_to_stream(&mut stream)
        .await
        .unwrap();
    match filter.read_from_stream(&mut stream).await.unwrap() {
        Message::Headers(headers) => {
            assert_eq!(headers.headers, expected[2..], "test for forked Headers")
        }
        messsage => panic!("Expected Headers, but got: {:?}", messsage),
    }

    node.stop().await;
}

#[tokio::test]
async fn get_data_blocks() {
    // ZG-CONFORMANCE-017, blocks portion
    //
    // The node responds to `GetData` requests with the appropriate transaction or block as requested by the peer.
    //
    // We test the following conditions:
    //  1. query for i=1..3 blocks
    //  2. a non-existing block
    //  3. a mixture of existing and non-existing blocks
    //
    // Test procedure:
    //  1. Create a node and seed it with the testnet chain
    //  2. Establish a peer node
    //  3. For each test case:
    //      a) send GetData
    //      b) receive a series Blocks
    //      c) assert Block received matches expectations
    //
    // The test currently fails for both Zebra and zcashd.
    //
    // Current behaviour:
    //
    //  zcashd: Ignores non-existing block requests, we expect `NotFound` to be sent but it never does (both in cases 2 and 3).
    //
    //  zebra: does not support seeding as yet, and therefore cannot perform this test.

    // Create a node with knowledge of the initial three testnet blocks
    let mut node: Node = Default::default();
    node.initial_action(Action::SeedWithTestnetBlocks {
        socket_addr: new_local_addr(),
        block_count: 3,
    })
    .log_to_stdout(true)
    .start()
    .await;

    // block headers and hashes
    let blocks = vec![
        Box::new(Block::testnet_genesis()),
        Box::new(Block::testnet_1()),
        Box::new(Block::testnet_2()),
    ];

    let inv_blocks = blocks
        .iter()
        .map(|block| block.inv_hash())
        .collect::<Vec<_>>();

    // Establish a peer node
    let mut stream = initiate_handshake(node.addr()).await.unwrap();
    let filter = MessageFilter::with_all_auto_reply();

    // Query for the first i blocks
    for i in 0..blocks.len() {
        Message::GetData(Inv::new(inv_blocks[..=i].to_vec()))
            .write_to_stream(&mut stream)
            .await
            .unwrap();
        // Expect the i blocks
        for j in 0..=i {
            match filter.read_from_stream(&mut stream).await.unwrap() {
                Message::Block(block) => assert_eq!(block, blocks[j], "run {}, {}", i, j),
                messsage => panic!("Expected Block, but got: {:?}", messsage),
            }
        }
    }

    // Query for a non-existant block
    let non_existant = InvHash::new(ObjectKind::Block, Hash::new([17; 32]));
    let non_existant_inv = Inv::new(vec![non_existant]);
    Message::GetData(non_existant_inv.clone())
        .write_to_stream(&mut stream)
        .await
        .unwrap();
    match filter.read_from_stream(&mut stream).await.unwrap() {
        Message::NotFound(not_found) => assert_eq!(not_found, non_existant_inv),
        messsage => panic!("Expected NotFound, but got: {:?}", messsage),
    }

    // Query a mixture of existing and non-existing blocks
    let mut mixed_blocks = inv_blocks;
    mixed_blocks.insert(1, non_existant);
    mixed_blocks.push(non_existant);

    let expected = vec![
        Message::Block(Box::new(Block::testnet_genesis())),
        Message::NotFound(non_existant_inv.clone()),
        Message::Block(Box::new(Block::testnet_1())),
        Message::Block(Box::new(Block::testnet_2())),
        Message::NotFound(non_existant_inv),
    ];

    Message::GetData(Inv::new(mixed_blocks))
        .write_to_stream(&mut stream)
        .await
        .unwrap();

    for expect in expected {
        let message = filter.read_from_stream(&mut stream).await.unwrap();
        assert_eq!(message, expect);
    }

    node.stop().await;
}

#[allow(dead_code)]
async fn unsolicitation_listener() {
    let mut node: Node = Default::default();
    node.start().await;

    let mut peer_stream = initiate_handshake(node.addr()).await.unwrap();

    let auto_responder = MessageFilter::with_all_auto_reply().enable_logging();

    for _ in 0usize..10 {
        let result = timeout(
            Duration::from_secs(5),
            auto_responder.read_from_stream(&mut peer_stream),
        )
        .await;

        match result {
            Err(elapsed) => println!("Timeout after {}", elapsed),
            Ok(Ok(message)) => println!("Received unfiltered message: {:?}", message),
            Ok(Err(err)) => println!("Error receiving message: {:?}", err),
        }
    }

    node.stop().await;
}
