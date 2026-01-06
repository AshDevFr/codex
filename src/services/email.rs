use crate::config::EmailConfig;
use anyhow::{Context, Result};
use lettre::message::{header::ContentType, Mailbox};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use std::sync::Arc;

#[derive(Clone)]
pub struct EmailService {
    pub config: Arc<EmailConfig>,
}

impl EmailService {
    pub fn new(config: EmailConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    pub async fn send_verification_email(
        &self,
        to_email: &str,
        to_name: &str,
        verification_token: &str,
    ) -> Result<()> {
        let verification_url = format!(
            "{}/auth/verify-email?token={}",
            self.config.verification_url_base, verification_token
        );

        let html_body = self.create_verification_email_html(to_name, &verification_url);
        let text_body = self.create_verification_email_text(to_name, &verification_url);

        self.send_email(
            to_email,
            "Verify your email address",
            &html_body,
            &text_body,
        )
        .await
    }

    pub async fn send_email(
        &self,
        to_email: &str,
        subject: &str,
        html_body: &str,
        _text_body: &str,
    ) -> Result<()> {
        let from_mailbox: Mailbox = format!(
            "{} <{}>",
            self.config.smtp_from_name, self.config.smtp_from_email
        )
        .parse()
        .context("Failed to parse from email address")?;

        let to_mailbox: Mailbox = to_email
            .parse()
            .context("Failed to parse recipient email address")?;

        let email = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html_body.to_string())
            .context("Failed to build email message")?;

        // Build SMTP transport
        let creds = Credentials::new(
            self.config.smtp_username.clone(),
            self.config.smtp_password.clone(),
        );

        let mailer = SmtpTransport::relay(&self.config.smtp_host)
            .context("Failed to create SMTP transport")?
            .credentials(creds)
            .port(self.config.smtp_port)
            .build();

        // Send the email
        mailer
            .send(&email)
            .context("Failed to send email via SMTP")?;

        tracing::info!("Verification email sent to {}", to_email);
        Ok(())
    }

    fn create_verification_email_html(&self, to_name: &str, verification_url: &str) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Verify Your Email</title>
</head>
<body style="font-family: Arial, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <div style="background-color: #f4f4f4; border-radius: 5px; padding: 20px; margin-bottom: 20px;">
        <h1 style="color: #444; margin-top: 0;">Welcome to Codex!</h1>
        <p>Hi {},</p>
        <p>Thank you for registering with Codex. To complete your registration and activate your account, please verify your email address by clicking the button below:</p>
        <div style="text-align: center; margin: 30px 0;">
            <a href="{}" style="background-color: #007bff; color: white; padding: 12px 30px; text-decoration: none; border-radius: 5px; display: inline-block; font-weight: bold;">Verify Email Address</a>
        </div>
        <p style="color: #666; font-size: 14px;">If the button doesn't work, you can also copy and paste this link into your browser:</p>
        <p style="word-break: break-all; color: #007bff; font-size: 14px;">{}</p>
        <p style="color: #666; font-size: 14px; margin-top: 30px;">This verification link will expire in {} hours.</p>
        <p style="color: #666; font-size: 14px;">If you didn't create an account with Codex, you can safely ignore this email.</p>
    </div>
    <div style="text-align: center; color: #999; font-size: 12px;">
        <p>&copy; 2026 Codex. All rights reserved.</p>
    </div>
</body>
</html>"#,
            to_name,
            verification_url,
            verification_url,
            self.config.verification_token_expiry_hours
        )
    }

    fn create_verification_email_text(&self, to_name: &str, verification_url: &str) -> String {
        format!(
            r#"Welcome to Codex!

Hi {},

Thank you for registering with Codex. To complete your registration and activate your account, please verify your email address by visiting the following link:

{}

This verification link will expire in {} hours.

If you didn't create an account with Codex, you can safely ignore this email.

---
© 2026 Codex. All rights reserved.
"#,
            to_name, verification_url, self.config.verification_token_expiry_hours
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> EmailConfig {
        EmailConfig {
            smtp_host: "localhost".to_string(),
            smtp_port: 587,
            smtp_username: "test".to_string(),
            smtp_password: "test".to_string(),
            smtp_from_email: "test@example.com".to_string(),
            smtp_from_name: "Test".to_string(),
            verification_token_expiry_hours: 24,
            verification_url_base: "http://localhost:8080".to_string(),
        }
    }

    #[test]
    fn test_create_verification_email_html() {
        let config = create_test_config();
        let service = EmailService::new(config);
        let html = service.create_verification_email_html(
            "John Doe",
            "http://localhost:8080/auth/verify-email?token=abc123",
        );

        assert!(html.contains("John Doe"));
        assert!(html.contains("http://localhost:8080/auth/verify-email?token=abc123"));
        assert!(html.contains("24 hours"));
    }

    #[test]
    fn test_create_verification_email_text() {
        let config = create_test_config();
        let service = EmailService::new(config);
        let text = service.create_verification_email_text(
            "John Doe",
            "http://localhost:8080/auth/verify-email?token=abc123",
        );

        assert!(text.contains("John Doe"));
        assert!(text.contains("http://localhost:8080/auth/verify-email?token=abc123"));
        assert!(text.contains("24 hours"));
    }
}
