import QtQuick 2.15
import QtQuick.Layouts 1.0
import QtQuick.Controls 2.15

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
            maxTemp = Math.max(maxTemp, gpuTemp);
        }
        return Logic.iconForTemp(maxTemp);
    }
    Plasmoid.toolTipMainText: {
        return "CPU: " + Logic.stringForTemp(A15KB.Controller.cpuTemp);
    }
    Plasmoid.toolTipSubText: {
        return "GPU: " + Logic.stringForTemp(A15KB.Controller.gpuTemp);
    }

    Plasmoid.fullRepresentation: ColumnLayout {
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
            ButtonGroup { id: radioGroup }

            PlasmaComponents.RadioButton {
                id: quietFanBtn
                text: "Quiet"
                ButtonGroup.group: radioGroup
            }
            PlasmaComponents.RadioButton {
                id: normalFanBtn
                text: "Normal"
                ButtonGroup.group: radioGroup
            }
            PlasmaComponents.RadioButton {
                id: gamingFanBtn
                text: "Gaming"
                ButtonGroup.group: radioGroup
            }

            RowLayout {
                spacing: PlasmaCore.Units.smallSpacing

                PlasmaComponents.RadioButton {
                    id: fixedFanBtn
                    text: "Custom: "
                    ButtonGroup.group: radioGroup
                }

                PlasmaComponents.Label {
                    text: Logic.stringForPercent(fixedFanSlider.value, 1);
                    color: {
                        if (fixedFanBtn.checked){
                            PlasmaCore.Theme.neutralTextColor
                        } else {
                            PlasmaCore.Theme.disabledTextColor
                        }
                    }
                }
            }
            ColumnLayout {
                Layout.preferredWidth: 200
                spacing: PlasmaCore.Units.smallSpacing
                transform: Translate {x: 20}
                
                PlasmaComponents.Slider {
                    id: fixedFanSlider
                    from: A15KB.Controller.fixedFanSpeedMin
                    to: A15KB.Controller.fixedFanSpeedMax
                    value: 0.5
                }
                RowLayout {
                    PlasmaComponents.Label {
                        Layout.fillWidth: true
                        text: Math.floor(A15KB.Controller.fixedFanSpeedMin * 100) + "%"
                        color: PlasmaCore.Theme.disabledTextColor
                    }
                    PlasmaComponents.Label {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: Math.floor(A15KB.Controller.fixedFanSpeedMax * 100) + "%"
                        color: PlasmaCore.Theme.disabledTextColor
                    }
                }
            }
            
            Connections {
                target: A15KB.Controller
                function onFanStateChanged(fanMode, fixedFanSpeed) {
                    let btns = [quietFanBtn, normalFanBtn, gamingFanBtn, fixedFanBtn];
                    if (!btns[fanMode].checked) {
                        btns[fanMode].toggle();
                    }
                    fixedFanSlider.value = fixedFanSpeed;
                }
            }
        }
        Rectangle {
            Layout.fillHeight: true
        }
    }
}