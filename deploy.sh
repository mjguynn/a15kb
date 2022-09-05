set -e
PLUGIN_DIR=/usr/lib/qt/qml/com/offbyond/a15kb
MODULE_DIR=~/.local/share/plasma/plasmoids/a15kb-fans

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

echo "=== Building QML plugin ==="
cargo build --release --example a15kb-qml-plugin

echo "=== Installing QML plugin ==="
echo "=== TODO: Is there another way to load a QML module from KDE? ==="
sudo mkdir -p $PLUGIN_DIR
sudo cp ./target/release/examples/liba15kb_qml_plugin.so $PLUGIN_DIR/liba15kb_qml_plugin.so
cat << EOF | sudo tee $PLUGIN_DIR/qmldir
module com.offbyond.a15kb
plugin a15kb_qml_plugin
EOF

echo "=== Deploying module to $MODULE_DIR ==="
rm -rf $MODULE_DIR/
cp -r ./plasmoids/a15kb-fans/ $MODULE_DIR/

echo "=== Done ==="