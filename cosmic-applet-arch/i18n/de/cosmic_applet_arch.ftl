up-to-date = Dein System ist auf dem neuesten Stand.
updates-available = { $numberUpdates ->
    [one] 1 { $updateSource }-Update verfügbar
   *[other] { $numberUpdates } { $updateSource }-Updates verfügbar
}
updates-available-with-error = { $numberUpdates ->
    [one] 1+ { $updateSource }-Update(s) verfügbar (Fehler beim letzten Aktualisieren)
   *[other] { $numberUpdates }+ { $updateSource }-Updates verfügbar (Fehler beim letzten Aktualisieren)
}
no-updates-available = Keine Updates verfügbar.
error-checking-updates = Fehler beim Prüfen von { $updateSource }-Updates

news = Neuigkeiten seit dem letzten Update – Zum Löschen klicken
no-news = Keine Neuigkeiten seit dem letzten Update.
error-checking-news = Fehler beim Abrufen der Neuigkeiten

loading = Wird geladen...
last-checked = Zuletzt geprüft: { $dateTime } – Zum Aktualisieren klicken
n-more = …und { $n } weitere.
