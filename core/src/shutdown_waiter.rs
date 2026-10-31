use tokio::signal::unix::{SignalKind, signal};
use tokio_util::sync::CancellationToken;

pub async fn wait_for_shutdown(ct: CancellationToken) {
    tokio::select! {
        biased;
        _ = ct.cancelled() => {},
        signal = wait_signal() => {
            tracing::info!("received {:?} signal, exiting", signal);
            ct.cancel();
        }
    }
}

#[derive(Debug)]
pub enum ExitSignal {
    SigTerm,
    SigInt,
    CouldNotAcquireWaitSignal,
}

pub async fn wait_signal() -> ExitSignal {
    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(v) => v,
        Err(_) => return ExitSignal::CouldNotAcquireWaitSignal,
    };
    let mut sigint = match signal(SignalKind::interrupt()) {
        Ok(v) => v,
        Err(_) => return ExitSignal::CouldNotAcquireWaitSignal,
    };

    tokio::select! {
        _ = sigterm.recv() => ExitSignal::SigTerm,
        _ = sigint.recv() => ExitSignal::SigInt,
    }
}
