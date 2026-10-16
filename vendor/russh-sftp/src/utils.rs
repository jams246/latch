use bytes::Bytes;
use chrono::{DateTime, Utc};
use std::time::SystemTime;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::Error;

pub(crate) fn unix(time: SystemTime) -> u32 {
    DateTime::<Utc>::from(time).timestamp() as u32
}

pub(crate) async fn read_packet<S: AsyncRead + Unpin>(
    stream: &mut S,
    max_length: u32,
) -> Result<Bytes, Error> {
    let length = stream.read_u32().await?;
    if length > max_length {
        return Err(Error::BadMessage("packet length limit exceeded".to_owned()));
    }

    let mut buf = vec![0; length as usize];
    stream.read_exact(&mut buf).await?;

    Ok(Bytes::from(buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn oversized_frame_fails_before_reading_or_allocating_the_body() {
        let header = u32::MAX.to_be_bytes();
        assert!(matches!(
            read_packet(&mut header.as_slice(), 262144).await,
            Err(Error::BadMessage(_))
        ));
    }

    #[tokio::test]
    async fn truncated_frame_fails_without_decoding_partial_data() {
        assert!(read_packet(&mut [0, 0, 0, 16, 1].as_slice(), 262144)
            .await
            .is_err());
    }
}
