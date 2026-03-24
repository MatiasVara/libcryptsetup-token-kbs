# libcryptsetup-token-tdx-kbs

LUKS2 external token handler for unlocking encrypted volumes using attestation
via a Key Broker Service (KBS). This has been tested using TDX and a QGS service.

This plugin performs the full attestation flow inline: it reads the
KBS URL from SMBIOS OEM strings, calls `trustee-attester` to obtain
the LUKS passphrase, and returns it directly to cryptsetup. No
external services or file-based handoffs are needed.

Inspired by [cvminjector](https://gitlab.com/berrange/cvminjector),
which uses a systemd service to inject LUKS keys into confidential VMs
via KBS attestation. This project replaces that external service
approach with a native LUKS2 token plugin, letting cryptsetup handle
key retrieval directly during volume activation.

## Requirements

- `trustee-attester` must be installed and available in `$PATH`. The
  plugin invokes it at runtime to perform the attestation handshake
  with the KBS. On Fedora it can be installed from Koji:

```bash
sudo dnf install https://kojipkgs.fedoraproject.org/packages/trustee-guest-components/0.17.0/3.fc44/x86_64/trustee-guest-components-0.17.0-3.fc44.x86_64.rpm
```

## Build

```bash
sudo dnf install cryptsetup-devel cargo
make
```

## Install

```bash
sudo make install
```

## Usage

### Register the token in a LUKS2 device

When SMBIOS OEM strings provide the KBS configuration (see
[SMBIOS initdata format](#smbios-initdata-format)), register a minimal
token:

```bash
cryptsetup token import /dev/vda4 <<< '{"type":"tdx-kbs","keyslots":["0"]}'
```

### Register the token with embedded KBS parameters

If SMBIOS OEM strings are not available, the KBS URL and resource path
can be embedded directly in the token JSON using `trustee.kbs.url` and
`trustee.kbs.resource`:

```bash
cryptsetup token import /dev/vda4 <<< '{"type":"tdx-kbs","keyslots":["0"],"trustee.kbs.url":"http://kbs-service:8080","trustee.kbs.resource":"default/my-vm/root"}'
```

When these fields are present the plugin uses them directly; otherwise
it falls back to reading SMBIOS OEM strings.

### Verify

```bash
cryptsetup luksDump /dev/vda4 | grep -A3 Token
```

## How it works

1. `systemd-cryptsetup` finds a LUKS2 device with a `tdx-kbs` token.
2. It loads this plugin (`libcryptsetup-token-tdx-kbs.so`).
3. The plugin reads SMBIOS OEM strings to find `initdata.toml`
   (base64-encoded, containing KBS URL and resource path).
4. It calls `trustee-attester` to perform TDX attestation against
   the KBS and retrieve the encrypted key.
5. The key is returned to cryptsetup, which unlocks the volume.

If the network is not yet ready (early initrd), the plugin retries
with exponential backoff for up to 120 seconds.

### SMBIOS initdata format

The VMM must supply an `initdata.toml` document as a base64-encoded
SMBIOS OEM string (Type 11). The plugin iterates over all OEM strings
and uses the first one that decodes to a valid TOML with the required
fields.

Expected TOML structure:

```toml
algorithm = "sha384"
version = "0.1.0"

[data]
"trustee.kbs.url" = "http://kbs-service:8080"
"trustee.kbs.resource" = "default/<vm-uuid>/root"
```

- `algorithm` and `version` are mandatory header fields.
- `trustee.kbs.url` is the URL of the Key Broker Service.
- `trustee.kbs.resource` is the KBS resource path in the format
  `<repository>/<type>/<tag>`.

With KubeVirt / QEMU, the base64-encoded TOML is typically passed via
`-smbios type=11,value=<base64>`.

### KBS resource format

The resource stored in the KBS must be a base64-encoded `confdata.toml`
document with the LUKS passphrase:

```toml
version = "0.1.0"

[data]
"io.cryptsetup.key.text.root" = "my-luks-passphrase"
```

- `version` must be `"0.1.0"`.
- The key name must start with `io.cryptsetup.key.text.`. The suffix
  (e.g. `root`) is informational.
- The value is the LUKS passphrase as a plain-text string.

### Integration with systemd-repart

`systemd-repart` can automatically register the `tdx-kbs` token when
creating an encrypted partition at first boot. This requires a patched
version of `systemd-repart` that supports the `EncryptToken=` option.

Create a repart definition in `/usr/lib/repart.d/10-root.conf`:

```ini
[Partition]
Type=root
Format=ext4
Encrypt=key-file
EncryptToken=tdx-kbs
CopyFiles=/
SizeMinBytes=3G
SizeMaxBytes=10G
```

The `EncryptToken=tdx-kbs` value tells `systemd-repart` to register a
LUKS2 token of type `tdx-kbs` in the new partition header. The token
type name must match the plugin library suffix:
`libcryptsetup-token-<name>.so` — in this case `tdx-kbs`.

On first boot, `systemd-repart` needs a key file to encrypt the
partition (`Encrypt=key-file`). A companion binary, `repart-kbs-helper`
(included in this crate), fetches the key from the KBS via attestation
and writes it to a file. It is typically run as an `ExecStartPre` in a
`systemd-repart.service` drop-in:

```ini
# /etc/systemd/system/systemd-repart.service.d/kbs-key.conf
[Unit]
Wants=network-online.target
After=network-online.target

[Service]
ExecStartPre=/bin/sh -c 'mkdir -p /run/cryptsetup-keys.d && /usr/libexec/repart-kbs-helper > /run/cryptsetup-keys.d/root.key'
ExecStart=
ExecStart=systemd-repart --key-file=/run/cryptsetup-keys.d/root.key --dry-run=no
```

After first boot, `systemd-repart` should be disabled (e.g. from a
post-repart script) since the partition already exists.

### TLS certificate

If `/etc/trustee-attester/kbs-ca.pem` exists, the plugin passes it to
`trustee-attester` via `--cert-file` so that TLS connections to the
KBS are verified against your own CA. This is useful when the KBS uses
a self-signed or private CA certificate.

## License

GPL-3.0-or-later
