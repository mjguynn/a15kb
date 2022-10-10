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

        TemperatureContainer {}
        FanControls {}
    }
}