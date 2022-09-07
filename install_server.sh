echo "=== Stopping existing service ==="
sudo systemctl stop a15kb.service
set -e

echo "=== Building server ==="
cargo build --release --bin a15kb

echo "=== Installing server  ==="
sudo cp ./target/release/a15kb /usr/sbin/a15kb

echo "=== Registering Systemd service === "
cat << EOF | sudo tee /etc/systemd/system/a15kb.service
[Unit]
Description=Aero 15 KB hardware control

[Service]
Type=dbus
ExecStart=/usr/sbin/a15kb
User=root
BusName=com.offbyond.a15kb
Restart=on-failure

[Install]
Alias=dbus-com.offbyond.a15kb.service
EOF

echo "=== Registering D-Bus service ==="
cat << EOF | sudo tee /usr/share/dbus-1/system-services/com.offbyond.a15kb.service
[D-BUS Service]
Names=com.offbyond.a15kb
Exec=/bin/false
User=root
SystemdService=dbus-com.offbyond.a15kb.service
EOF

echo "=== Updating D-Bus security policy ==="
cat << EOF | sudo tee /usr/share/dbus-1/system.d/com.offbyond.a15kb.conf
<?xml version="1.0"?> <!--*-nxml-*-->
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
	"http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
	<policy user="root">
		<allow own="com.offbyond.a15kb"/>
		<allow send_destination="com.offbyond.a15kb"/>
		<allow receive_sender="com.offbyond.a15kb"/>
	</policy>
	<policy context="default">
		<allow send_destination="com.offbyond.a15kb"/>
		<allow receive_sender="com.offbyond.a15kb"/>
	</policy>
</busconfig>
EOF

echo "=== Enabling and starting service ==="
sudo systemctl enable a15kb.service
sudo systemctl start a15kb.service

echo "=== Done ==="
