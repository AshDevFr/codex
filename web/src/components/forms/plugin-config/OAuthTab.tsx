import {
  Alert,
  Button,
  Code,
  CopyButton,
  Group,
  Paper,
  PasswordInput,
  Stack,
  Text,
  TextInput,
  Tooltip,
} from "@mantine/core";
import { IconCopy, IconInfoCircle } from "@tabler/icons-react";
import type { PluginDto } from "@/api/plugins";
import type { PluginConfigForm } from "./types";

interface OAuthTabProps {
  plugin: PluginDto;
  form: PluginConfigForm;
}

export function OAuthTab({ plugin, form }: OAuthTabProps) {
  return (
    <Stack gap="md">
      {plugin.manifest?.adminSetupInstructions && (
        <Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
          <Text size="sm" style={{ whiteSpace: "pre-line" }}>
            {plugin.manifest.adminSetupInstructions}
          </Text>
        </Alert>
      )}

      <Paper p="sm" withBorder>
        <Stack gap="xs">
          <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
            OAuth Callback URL
          </Text>
          <Text size="xs" c="dimmed">
            Set this as the redirect URL in your OAuth provider settings.
          </Text>
          <Group gap="xs">
            <Code style={{ fontSize: 12, flex: 1 }}>
              {`${window.location.origin}/api/v1/user/plugins/oauth/callback`}
            </Code>
            <CopyButton
              value={`${window.location.origin}/api/v1/user/plugins/oauth/callback`}
            >
              {({ copied, copy }) => (
                <Tooltip label={copied ? "Copied" : "Copy"} withArrow>
                  <Button
                    size="compact-xs"
                    variant="subtle"
                    onClick={copy}
                    leftSection={<IconCopy size={14} />}
                  >
                    {copied ? "Copied" : "Copy"}
                  </Button>
                </Tooltip>
              )}
            </CopyButton>
          </Group>
        </Stack>
      </Paper>

      <TextInput
        label="OAuth Client ID"
        placeholder="Enter the client ID from your OAuth provider"
        description="Required for OAuth flow. Users cannot connect via OAuth without this."
        {...form.getInputProps("oauthClientId")}
      />

      <PasswordInput
        label="OAuth Client Secret"
        placeholder="Enter the client secret (optional for some providers)"
        description="Some providers require a client secret for token exchange."
        {...form.getInputProps("oauthClientSecret")}
      />

      {plugin.manifest?.oauth && (
        <Paper p="sm" withBorder bg="var(--mantine-color-dark-7)">
          <Stack gap="xs">
            <Text size="xs" fw={500}>
              OAuth Endpoints (from manifest)
            </Text>
            <Group gap="xl">
              <div>
                <Text size="xs" c="dimmed">
                  Authorization URL
                </Text>
                <Text size="xs" ff="monospace">
                  {plugin.manifest.oauth.authorizationUrl}
                </Text>
              </div>
              <div>
                <Text size="xs" c="dimmed">
                  Token URL
                </Text>
                <Text size="xs" ff="monospace">
                  {plugin.manifest.oauth.tokenUrl}
                </Text>
              </div>
            </Group>
            {plugin.manifest.oauth.scopes &&
              plugin.manifest.oauth.scopes.length > 0 && (
                <div>
                  <Text size="xs" c="dimmed">
                    Scopes
                  </Text>
                  <Text size="xs" ff="monospace">
                    {plugin.manifest.oauth.scopes.join(", ")}
                  </Text>
                </div>
              )}
          </Stack>
        </Paper>
      )}
    </Stack>
  );
}
