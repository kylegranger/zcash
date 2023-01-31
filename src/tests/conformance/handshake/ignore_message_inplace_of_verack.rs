//! Contains test cases which cover ZG-CONFORMANCE-005 and ZG-CONFORMANCE-006.
//!
//! The node doesn't terminate the connection on non-`Verack` messages as a response to initial `Verack` it sent.

use std::io;

use crate::{
    protocol::{
        message::Message,
        payload::{
            block::{Block, LocatorHashes},
            Addr, Hash, Inv, Nonce,
        },
    },
    setup::node::{Action, Node},
    tools::{synthetic_node::SyntheticNode, LONG_TIMEOUT, RECV_TIMEOUT},
};

mod when_node_receives_connection {
    //! Contains test cases which cover ZG-CONFORMANCE-005.
    use super::*;

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c005_t1_GET_ADDR() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::GetAddr).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c005_t2_MEMPOOL() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::MemPool).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c005_t3_PING() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Ping(Nonce::default()))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c005_t4_PONG() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Pong(Nonce::default()))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c005_t5_ADDR() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Addr(Addr::empty())).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c005_t6_GET_HEADERS() {
        // zcashd: pass
        // zebra:  pass
        let block_hash = Block::testnet_genesis().double_sha256().unwrap();
        let block_loc = LocatorHashes::new(vec![block_hash], Hash::zeroed());
        run_test_case(Message::GetHeaders(block_loc)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c005_t7_GET_BLOCKS() {
        // zcashd: pass
        // zebra:  pass
        let block_hash = Block::testnet_genesis().double_sha256().unwrap();
        let block_loc = LocatorHashes::new(vec![block_hash], Hash::zeroed());
        run_test_case(Message::GetBlocks(block_loc)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c005_t8_GET_DATA_BLOCK() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::GetData(block_inv)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c005_t9_GET_DATA_TX() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::GetData(Inv::new(vec![Block::testnet_genesis()
            .txs[0]
            .inv_hash()])))
        .await
        .unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c005_t10_INV() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::Inv(block_inv)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c005_t11_NOT_FOUND() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::NotFound(block_inv)).await.unwrap();
    }

    /// Checks that `message` doesn't terminate the connection when sent instead of [`Message::Version`], when the node
    /// receives the connection.
    async fn run_test_case(message: Message) -> io::Result<()> {
        // Spin up a node instance.
        let mut node = Node::new()?;
        node.initial_action(Action::WaitForConnection)
            .start()
            .await?;
        // Connect to the node, and exchange versions.
        let synthetic_node = SyntheticNode::builder()
            .with_version_exchange_handshake()
            .build()
            .await?;
        synthetic_node.connect(node.addr()).await?;

        // Send a non-verack message.
        // We expect the node to not disconnect before completing the handshake.
        synthetic_node.unicast(node.addr(), message)?;

        // Send Verack.
        synthetic_node.unicast(node.addr(), Message::Verack)?;

        // This is only set post-handshake (if enabled).
        assert!(synthetic_node.is_connected(node.addr()));

        // Gracefully shut down the nodes.
        synthetic_node.shut_down().await;
        node.stop()?;

        Ok(())
    }
}

mod when_node_initiates_connection {
    //! Contains test cases which cover ZG-CONFORMANCE-006.
    use super::*;

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c006_t1_GET_ADDR() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::GetAddr).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c006_t2_MEMPOOL() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::MemPool).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c006_t3_PING() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Ping(Nonce::default()))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c006_t4_PONG() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Pong(Nonce::default()))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c006_t5_ADDR() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Addr(Addr::empty())).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c006_t6_GET_HEADERS() {
        // zcashd: pass
        // zebra:  pass
        let block_hash = Block::testnet_genesis().double_sha256().unwrap();
        let block_loc = LocatorHashes::new(vec![block_hash], Hash::zeroed());
        run_test_case(Message::GetHeaders(block_loc)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c006_t7_GET_BLOCKS() {
        // zcashd: pass
        // zebra:  pass
        let block_hash = Block::testnet_genesis().double_sha256().unwrap();
        let block_loc = LocatorHashes::new(vec![block_hash], Hash::zeroed());
        run_test_case(Message::GetBlocks(block_loc)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c006_t8_GET_DATA_BLOCK() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::GetData(block_inv)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c006_t9_GET_DATA_TX() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::GetData(Inv::new(vec![Block::testnet_genesis()
            .txs[0]
            .inv_hash()])))
        .await
        .unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c006_t10_INV() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::Inv(block_inv)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c006_t11_NOT_FOUND() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::NotFound(block_inv)).await.unwrap();
    }

    /// Checks that `message` doesn't terminate the connection when sent instead of [`Message::Verack`], when the node
    /// initiates the connection and instead ignores the message.
    async fn run_test_case(message: Message) -> io::Result<()> {
        // Create a SyntheticNode and store its listening address.
        // Enable version-only handshake
        let mut synthetic_node = SyntheticNode::builder()
            .with_version_exchange_handshake()
            .build()
            .await?;

        // Spin up a node instance which will connect to our SyntheticNode.
        let mut node = Node::new()?;
        node.initial_peers(vec![synthetic_node.listening_addr()])
            .start()
            .await?;

        // Wait for the node to establish the connection.
        // This will result in a connection in which the Version's have
        // already been exchanged.
        let node_addr =
            tokio::time::timeout(LONG_TIMEOUT, synthetic_node.wait_for_connection()).await?;

        // Send a non-verack message.
        // We expect the node to not disconnect before completing the handshake.
        synthetic_node.unicast(node_addr, message)?;

        // Send Verack.
        synthetic_node.unicast(node_addr, Message::Verack)?;

        // Read Verack.
        match synthetic_node.recv_message_timeout(RECV_TIMEOUT).await {
            Ok((_, Message::Verack)) => Ok(()),
            Ok((_, unexpected)) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Message was not ignored, received {unexpected}"),
            )),
            Err(_timeout) if !synthetic_node.is_connected(node.addr()) => Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Connection terminated",
            )),
            Err(err) => Err(err),
        }?;

        // Gracefully shut down the nodes.
        synthetic_node.shut_down().await;
        node.stop()?;

        Ok(())
    }
}
