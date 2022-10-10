import QtQuick 2.15
import QtQuick.Layouts 1.0

import org.kde.plasma.components 3.0 as PlasmaComponents
import org.kde.plasma.core 2.1 as PlasmaCore
import org.kde.plasma.plasmoid 2.0

import com.offbyond.a15kb 1.0 as A15KB

import "logic.js" as Logic

Item {
    id: root

    Plasmoid.icon: {
        let maxTemp = A15KB.Controller.cpuTemp;
        let gpuTemp = A15KB.Controller.gpuTemp;
        if (gpuTemp != 0) {
            maxTemp = Math.max(avgTemp + gpuTemp);
        }
        return Logic.iconForTemp(avgTemp);
    }
    Plasmoid.toolTipMainText: {
        return "CPU: " + Logic.stringForTemp(A15KB.Controller.cpuTemp);
    }
    Plasmoid.toolTipSubText: {
        return "GPU: " + Logic.stringForTemp(A15KB.Controller.gpuTemp);
    }

    Plasmoid.fullRepresentation: ColumnLayout {
        anchors.fill: parent
        spacing: PlasmaCore.Units.gridUnit

        Section {
            title: i18n("Hardware Temperatures")
            TemperatureDisplay {
                deviceName: i18n("CPU")
                temperature: A15KB.Controller.cpuTemp
            }
            TemperatureDisplay {
                deviceName: i18n("GPU")
                temperature: A15KB.Controller.gpuTemp
            }
        }
        Section {
            title: i18n("Fan Configuration")
            PlasmaComponents.RadioButton {
                id: quietFanBtn
                text: "Quiet"
            }
            PlasmaComponents.RadioButton {
                id: normalFanBtn
                text: "Normal"
            }
            PlasmaComponents.RadioButton {
                id: gamingFanBtn
                text: "Gaming"
            }
            PlasmaComponents.RadioButton {
                id: fixedFanBtn
                text: "Custom"
            }
            PlasmaComponents.Slider {
                id: fixedFanSlider
                from: A15KB.Controller.fixedFanSpeedMin
                to: A15KB.Controller.fixedFanSpeedMax
                value: 0.5
            }
            PlasmaComponents.Label {
                id: debugLabel
            }
            Connections {
                target: A15KB.Controller
                function onFanStateChanged(fanMode, fixedFanSpeed) {
                    let btns = [quietFanBtn, normalFanBtn, gamingFanBtn, fixedFanBtn];
                    if (!btns[fanMode].checked) {
                        btns[fanMode].toggle();
                    }
                    debugLabel.text = "" + fanMode;
                    fixedFanSlider.value = fixedFanSpeed;
                }
            }
        }
    }
}