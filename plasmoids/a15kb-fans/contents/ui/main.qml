import QtQuick 2.0
import QtQuick.Layouts 1.0
import org.kde.plasma.components 3.0 as PlasmaComponents
import org.kde.plasma.plasmoid 2.0
import com.offbyond.a15kb 1.0 as A15KB

Item {
    function icon() {
        var cpu_temp = 90;
        if (cpu_temp < 40) {
            return "temperature-cold"
        } else if (cpu_temp < 65) {
            return "temperature-normal"
        } else {
            return "temperature-warm"
        }
    }
    Plasmoid.icon: icon()
    Plasmoid.fullRepresentation: ColumnLayout {
        PlasmaComponents.Label {
            text: A15KB.Controller.error
        }
        PlasmaComponents.RadioButton {
            text: "Quiet"
            onClicked: A15KB.Controller.set_fans_quiet()
            autoExclusive: true
        }
        PlasmaComponents.RadioButton {
            text: "Normal"
            checked: true
            autoExclusive: true
        }
        PlasmaComponents.RadioButton {
            text: "Gaming"
            autoExclusive: true
        }
        PlasmaComponents.RadioButton {
            text: "Custom"
            autoExclusive: true
            PlasmaComponents.Slider {
                id: slider
                Layout.fillWidth: true
                from: 0
                to: 100
                value: 50
                stepSize: 5
            }
        }
    }
}