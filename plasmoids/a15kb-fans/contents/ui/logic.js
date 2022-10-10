function iconForTemp(temp) {
    if (temp == 0) {
        return "offline";
    } else if (temp < 40) {
        return "temperature-cold";
    } else if (temp < 65) {
        return "temperature-normal";
    } else {
        return "temperature-warm";
    }
}
function stringForTemp(temp) {
    if (temp == 0) {
        return i18nc("The device is not reporting a temperature, which probably means it's not powered.", "Offline");
    } else {
        return `${temp}°C`
    }
}