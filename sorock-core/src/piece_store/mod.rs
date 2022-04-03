use crate::*;

pub mod mem;
pub mod sqlite;

#[norpc::service]
trait PieceStore {
    fn get_pieces(key: String, n: u8) -> anyhow::Result<Vec<(u8, Vec<u8>)>>;
    fn get_piece(loc: PieceLocator) -> anyhow::Result<Option<Vec<u8>>>;
    fn put_piece(loc: PieceLocator, data: Bytes) -> anyhow::Result<()>;
    fn delete_piece(loc: PieceLocator) -> anyhow::Result<()>;
    fn piece_exists(loc: PieceLocator) -> anyhow::Result<bool>;
    fn keys() -> anyhow::Result<Vec<String>>;
}
define_client!(PieceStore);

#[cfg(test)]
async fn test_piece_store(mut cli: piece_store::ClientT) -> anyhow::Result<()> {
    assert_eq!(cli.keys().await??.len(), 0);
    assert_eq!(cli.get_pieces("a".to_string(), 8).await??, vec![]);
    assert_eq!(
        cli.piece_exists(PieceLocator {
            key: "a".to_string(),
            index: 1
        })
        .await??,
        false
    );

    // put (a,1)
    cli.put_piece(
        PieceLocator {
            key: "a".to_string(),
            index: 1,
        },
        vec![0, 0, 0, 0].into(),
    )
    .await??;
    assert_eq!(cli.keys().await??.len(), 1);
    assert_eq!(cli.get_pieces("a".to_string(), 8).await??.len(), 1);
    assert_eq!(
        cli.piece_exists(PieceLocator {
            key: "a".to_string(),
            index: 1
        })
        .await??,
        true
    );
    assert_eq!(
        cli.piece_exists(PieceLocator {
            key: "a".to_string(),
            index: 0
        })
        .await??,
        false
    );

    // put (a,2)
    cli.put_piece(
        PieceLocator {
            key: "a".to_string(),
            index: 2,
        },
        vec![0, 0, 0, 0].into(),
    )
    .await??;
    assert_eq!(cli.keys().await??.len(), 1);
    assert_eq!(cli.get_pieces("a".to_string(), 8).await??.len(), 2);
    assert_eq!(
        cli.piece_exists(PieceLocator {
            key: "a".to_string(),
            index: 2
        })
        .await??,
        true
    );

    // put (b,3)
    cli.put_piece(
        PieceLocator {
            key: "b".to_string(),
            index: 3,
        },
        vec![0, 0, 0, 0].into(),
    )
    .await??;
    assert_eq!(cli.keys().await??.len(), 2);
    assert_eq!(
        cli.piece_exists(PieceLocator {
            key: "a".to_string(),
            index: 3
        })
        .await??,
        false
    );

    // delete (a,1)
    cli.delete_piece(PieceLocator {
        key: "a".to_string(),
        index: 1,
    })
    .await??;
    assert_eq!(cli.keys().await??.len(), 2);
    assert_eq!(cli.get_pieces("a".to_string(), 8).await??.len(), 1);

    // delete (a,2)
    cli.delete_piece(PieceLocator {
        key: "a".to_string(),
        index: 2,
    })
    .await??;
    assert_eq!(cli.keys().await??.len(), 1);
    assert_eq!(cli.get_pieces("a".to_string(), 8).await??.len(), 0);

    Ok(())
}
