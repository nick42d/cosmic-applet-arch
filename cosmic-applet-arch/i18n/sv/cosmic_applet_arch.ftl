up-to-date = Ditt system är uppdaterat.
updates-available = { $numberUpdates ->
                     [one] 1 { $updateSource } uppdatering tillgänglig
                     *[other] { $numberUpdates } { $updateSource } uppdateringar tillgängliga
}
updates-available-with-error = { $numberUpdates ->
    [one] 1+ { $updateSource } update(s) available (error when last refreshed)
   *[other] { $numberUpdates }+ { $updateSource } updates available (error when last refreshed)
}
no-updates-available = Inga uppdateringar tillgängliga.
error-checking-updates = Error checking { $updateSource } updates

news = Nyheter sedan senaste uppdateringen - klicka för att rensa
no-news = Inga nyheter sedan senaste uppdateringen.
error-checking-news = Error checking news

loading = Laddar...
last-checked = Senast kontrollerat: { $dateTime } - klicka för att uppdatera
n-more = ...och { $n } mer.
