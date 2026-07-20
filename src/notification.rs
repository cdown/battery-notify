use log::{error, trace};
use notify_rust::{Notification, NotificationHandle, Timeout, Urgency};

#[derive(Default)]
pub struct SingleNotification {
    hnd: Option<NotificationHandle>,
    summary: Option<String>,
}

impl SingleNotification {
    pub fn show(&mut self, summary: String, urgency: Urgency, timeout: i32) {
        if self.summary.as_ref() != Some(&summary) {
            self.close();
            trace!("Creating notification for {}", summary);
            self.hnd = Notification::default()
                .summary(&summary)
                .urgency(urgency)
                .timeout(Timeout::from(timeout))
                .show()
                .map_err(|err| error!("error showing notification: {err}"))
                .ok();
            self.summary = Some(summary)
        }
    }

    pub fn close(&mut self) {
        if let Some(hnd) = self.hnd.take() {
            if let Some(summary) = self.summary.take() {
                trace!("Closing notification for {}", summary);
            }
            hnd.close();
        }
    }
}

impl Drop for SingleNotification {
    fn drop(&mut self) {
        self.close();
    }
}

/// A `SingleNotification` pre-configured with a fixed summary, urgency, and timeout.
#[allow(dead_code)]
pub struct ConfiguredNotification {
    inner: SingleNotification,
    summary: String,
    urgency: Urgency,
    timeout: i32,
}

#[allow(dead_code)]
impl ConfiguredNotification {
    pub fn new(summary: impl Into<String>, urgency: Urgency, timeout: i32) -> Self {
        Self {
            inner: SingleNotification::default(),
            summary: summary.into(),
            urgency,
            timeout,
        }
    }

    pub fn show(&mut self) {
        self.inner
            .show(self.summary.clone(), self.urgency, self.timeout);
    }

    pub fn close(&mut self) {
        self.inner.close();
    }
}
