set -e
PLUGIN_DIR=/usr/lib/qt/qml/com/offbyond/a15kb
MODULE_DIR=~/.local/share/plasma/plasmoids/a15kb-fans
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
mkdir -p $MODULE_DIR/
cp -r ./plasmoids/a15kb-fans/ $MODULE_DIR/
