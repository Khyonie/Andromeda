# Andromeda

A web tool for provisioning and managing libvirt virtual machines.

<img width="2101" height="1013" alt="image-6" src="https://github.com/user-attachments/assets/ce82e687-42c2-4058-9cbb-2e311277ed1e" />

# Installation
1. Install required packages:
```shell
sudo pacman -Sy --needed \
  qemu-full libvirt virt-manager virt-install libguestfs cdrtools \
  cloud-utils guestfs-tools openssh pkgconf nodejs npm git
```
> [!NOTE]
> *Use the appropriate package manager for your distribution.*

Also install Cargo, Rust's package manager:
[official installation instructions](https://rust-lang.org/tools/install/)

2. Start libvirt:
```shell
sudo systemctl enable --now libvirtd.service
```

3. Create a virtual network bridge named "`br0`", [following this guide](https://linuxconfig.org/how-to-use-bridged-networking-with-libvirt-and-kvm).
4. Clone the repository:
```shell
git clone https://github.com/Khyonie/Andromeda.git andromeda-rs
cd andromeda-rs
```
5. Create a Discord application for user login, [following this guide](https://docs.discord.com/developers/topics/oauth2).
6. Configure the backend environment file:
```shell
cp .env.example .env
```
Set `ANDROMEDA_PUBLIC_ORIGIN` to your frontend's URL, and `DISCORD_CLIENT_ID` and `DISCORD_CLIENT_SECRET` from your Discord application.

Set `ANDROMEDA_ADMIN_DISCORD_IDS` to your Discord user ID.
> [!CAUTION]
> This user will have unrestricted access to every VM! Enabling Discord MFA is highly recommended.

7. Build and start the backend:
```shell
cargo build --release --locked

sudo ./target/release/andromeda
```

8. In a new shell session, build and start the frontend:
```shell
npm --prefix src-frontend ci
npm --prefix src-frontend run check
npm --prefix src-frontend run build
npm --prefix src-frontend run dev
```

9. Open the frontend in a browser, and log in with your Discord account.
10. Open the **Administration** tab in the sidebar. Set the default gateway for your network, and your SSH public key.
