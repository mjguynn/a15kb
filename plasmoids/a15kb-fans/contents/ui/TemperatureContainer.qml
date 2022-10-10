import QtQuick 2.15
import QtQuick.Layouts 1.2

import org.kde.plasma.components 3.0 as PlasmaComponents
import org.kde.plasma.core 2.1 as PlasmaCore
import org.kde.plasma.extras 2.0 as PlasmaExtras

import com.offbyond.a15kb 1.0 as A15KB

import "logic.js" as Logic

ColumnLayout {
    anchors {
        left: parent.left
        right: parent.right
    }
    spacing: PlasmaCore.Units.smallSpacing

    PlasmaExtras.Heading {
        text: i18n("Hardware Temperatures")
    }

    Separator {}
    
    ColumnLayout {
        transform: Translate {x: PlasmaCore.Units.smallSpacing * 4}

        TemperatureDisplay {
            deviceName: i18n("CPU")
            temperature: A15KB.Controller.cpuTemp
        }

        TemperatureDisplay {
            deviceName: i18n("GPU")
            temperature: A15KB.Controller.gpuTemp
        }
    }
}