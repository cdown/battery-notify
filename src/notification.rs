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
