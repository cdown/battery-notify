use log::{error, trace};
use notify_rust::{Notification, NotificationHandle, Timeout, Urgency};

#[derive(Default)]
pub struct SingleNotification {
    hnd: Option<NotificationHandle>,
    summary: String,
}

impl SingleNotification {
    pub fn show(&mut self, summary: String, urgency: Urgency, timeout: i32) {
        if self.summary != summary {
            self.close();
            self.summary = summary;
            trace!("Creating notification for {}", self.summary);
            self.hnd = Notification::default()
                .summary(&self.summary)
                .urgency(urgency)
                .timeout(Timeout::from(timeout))
                .show()
                .map_err(|err| error!("error showing notification: {err}"))
                .ok();
        }
    }

    pub fn close(&mut self) {
        if let Some(hnd) = self.hnd.take() {
            trace!("Closing notification for {}", self.summary);
            hnd.close();
        }
    }
}

impl Drop for SingleNotification {
    fn drop(&mut self) {
        self.close();
    }
}
