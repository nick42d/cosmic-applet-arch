up-to-date = Ditt system är uppdaterat.
updates-available = { $numberUpdates ->
                     [one] 1 { $updateSource } uppdatering tillgänglig
                     *[other] { $numberUpdates } { $updateSource } uppdateringar tillgängliga
}
updates-available-with-error = { $numberUpdates ->
    [one] 1+ { $updateSource } uppdatering(ar) tillgänglig(a) (fel vid senast uppdatering)
   *[other] { $numberUpdates }+ { $updateSource } uppdateringar tillgängliga (fel vid senast uppdatering)
}
no-updates-available = Inga uppdateringar tillgängliga.
error-checking-updates = Fel vid kontroll { $updateSource } uppdateringar

news = Nyheter sedan senaste uppdateringen - klicka för att rensa
no-news = Inga nyheter sedan senaste uppdateringen.
error-checking-news = Fel vid kontroll av nyheter

loading = Laddar...
last-checked = Senast kontrollerat: { $dateTime } - klicka för att uppdatera
n-more = ...och { $n } mer.
