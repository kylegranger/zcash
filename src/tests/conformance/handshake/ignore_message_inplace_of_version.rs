//! Contains test cases which cover ZG-CONFORMANCE-003 and ZG-CONFORMANCE-004.
//!
//! The node ignores non-version messages sent inplace of version.

use std::io;

use crate::{
    protocol::{
        message::Message,
        payload::{
            block::{Block, LocatorHashes},
            Addr, Hash, Inv, Nonce, Version,
        },
    },
    setup::node::{Action, Node},
    tools::{synthetic_node::SyntheticNode, LONG_TIMEOUT, RECV_TIMEOUT},
};

mod when_node_receives_connection {
    //! Contains test cases which cover ZG-CONFORMANCE-003.

    use super::*;

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c003_t1_GET_ADDR() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::GetAddr).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c003_t2_MEMPOOL() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::MemPool).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c003_t3_VERACK() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Verack).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c003_t4_PING() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Ping(Nonce::default()))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c003_t5_PONG() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Pong(Nonce::default()))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c003_t6_ADDR() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Addr(Addr::empty())).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c003_t7_GET_HEADERS() {
        // zcashd: pass
        // zebra:  pass
        let block_hash = Block::testnet_genesis().double_sha256().unwrap();
        let block_loc = LocatorHashes::new(vec![block_hash], Hash::zeroed());
        run_test_case(Message::GetHeaders(block_loc)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c003_t8_GET_BLOCKS() {
        // zcashd: pass
        // zebra:  pass
        let block_hash = Block::testnet_genesis().double_sha256().unwrap();
        let block_loc = LocatorHashes::new(vec![block_hash], Hash::zeroed());
        run_test_case(Message::GetBlocks(block_loc)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c003_t9_GET_DATA_BLOCK() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::GetData(block_inv)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c003_t10_GET_DATA_TX() {
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
    async fn c003_t11_INV() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::Inv(block_inv)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c003_t12_NOT_FOUND() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::NotFound(block_inv)).await.unwrap();
    }

    /// Checks that `message` gets ignored when sent instead of [`Message::Version`] when the node
    /// receives the connection.
    async fn run_test_case(message: Message) -> io::Result<()> {
        // Spin up a node instance.
        let mut node = Node::new()?;
        node.initial_action(Action::WaitForConnection)
            .start()
            .await?;
        // Connect to the node, don't handshake.
        let mut synthetic_node = SyntheticNode::builder().build().await?;
        synthetic_node.connect(node.addr()).await?;

        // Send a non-version message.
        synthetic_node.unicast(node.addr(), message)?;

        // Expect the node to ignore the previous message, verify by completing the handshake.
        // Send Version.
        synthetic_node.unicast(
            node.addr(),
            Message::Version(Version::new(synthetic_node.listening_addr(), node.addr())),
        )?;

        // Read Version.
        match synthetic_node.recv_message_timeout(RECV_TIMEOUT).await {
            Ok((_, Message::Version(..))) => Ok(()),
            Ok((_, unexpected)) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Message was not ignored. Instead of Version received {unexpected}"),
            )),
            Err(_timeout) if !synthetic_node.is_connected(node.addr()) => Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Connection terminated",
            )),
            Err(err) => Err(err),
        }?;

        // Send Verack.
        synthetic_node.unicast(node.addr(), Message::Verack)?;

        // Read Verack.
        match synthetic_node.recv_message_timeout(RECV_TIMEOUT).await {
            Ok((_, Message::Verack)) => Ok(()),
            Ok((_, unexpected)) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Message was not ignored. Instead of Verack received {unexpected}"),
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

mod when_node_initiates_connection {
    //! Contains test cases which cover ZG-CONFORMANCE-004.
    use super::*;

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c004_t1_GET_ADDR() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::GetAddr).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c004_t2_MEMPOOL() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::MemPool).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c004_t3_VERACK() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Verack).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c004_t4_PING() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Ping(Nonce::default()))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c004_t5_PONG() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Pong(Nonce::default()))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c004_t6_ADDR() {
        // zcashd: pass
        // zebra:  pass
        run_test_case(Message::Addr(Addr::empty())).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c004_t7_GET_HEADERS() {
        // zcashd: pass
        // zebra:  pass
        let block_hash = Block::testnet_genesis().double_sha256().unwrap();
        let block_loc = LocatorHashes::new(vec![block_hash], Hash::zeroed());
        run_test_case(Message::GetHeaders(block_loc)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c004_t8_GET_BLOCKS() {
        // zcashd: pass
        // zebra:  pass
        let block_hash = Block::testnet_genesis().double_sha256().unwrap();
        let block_loc = LocatorHashes::new(vec![block_hash], Hash::zeroed());
        run_test_case(Message::GetBlocks(block_loc)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c004_t9_GET_DATA_BLOCK() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::GetData(block_inv)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c004_t10_GET_DATA_TX() {
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
    async fn c004_t11_INV() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::Inv(block_inv)).await.unwrap();
    }

    #[tokio::test]
    #[allow(non_snake_case)]
    async fn c004_t12_NOT_FOUND() {
        // zcashd: pass
        // zebra:  pass
        let block_inv = Inv::new(vec![Block::testnet_genesis().inv_hash()]);
        run_test_case(Message::NotFound(block_inv)).await.unwrap();
    }

    /// Checks that `message` gets ignored when sent instead of [`Message::Version`] when the node
    /// initiates the connection.
    async fn run_test_case(message: Message) -> io::Result<()> {
        // Create a SyntheticNode and store its listening address.
        let mut synthetic_node = SyntheticNode::builder().build().await?;

        // Spin up a node instance which will connect to our SyntheticNode.
        let mut node = Node::new()?;
        node.initial_peers(vec![dbg!(synthetic_node.listening_addr())])
            .start()
            .await?;

        // Wait for the node to establish the connection.
        let node_addr =
            tokio::time::timeout(LONG_TIMEOUT, synthetic_node.wait_for_connection()).await?;

        // Send a non-version message.
        synthetic_node.unicast(node_addr, message)?;

        // Expect the node to ignore the previous message, verify by completing the handshake.
        // Send Version.
        synthetic_node.unicast(
            node_addr,
            Message::Version(Version::new(synthetic_node.listening_addr(), node_addr)),
        )?;

        // Read Version.
        match synthetic_node.recv_message_timeout(RECV_TIMEOUT).await {
            Ok((_, Message::Version(..))) => Ok(()),
            Ok((_, unexpected)) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Message was not ignored. Instead of Version received {unexpected}"),
            )),
            Err(_timeout) if !synthetic_node.is_connected(node.addr()) => Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Connection terminated",
            )),
            Err(err) => Err(err),
        }?;

        // Send Verack.
        synthetic_node.unicast(node_addr, Message::Verack)?;

        // Read Verack.
        match synthetic_node.recv_message_timeout(RECV_TIMEOUT).await {
            Ok((_, Message::Verack)) => Ok(()),
            Ok((_, unexpected)) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Message was not ignored. Instead of Verack received {unexpected}"),
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
