up-to-date = Your system is up to date.
updates-available = { $numberUpdates ->
    [one] 1 { $updateSource } update available
   *[other] { $numberUpdates } { $updateSource } updates available
}
updates-available-with-error = { $numberUpdates ->
    [one] 1+ { $updateSource } update(s) available (error when last refreshed)
   *[other]+ { $numberUpdates } { $updateSource } updates available (error when last refreshed)
}
no-updates-available = No updates available.
error-checking-updates = Error checking { $updateSource }

news = News since last update - Click to clear
no-news = No news since last update.
error-checking-news = Error checking news

loading = Loading...
last-checked = Last checked: { $dateTime } - Click to refresh
n-more = ...and { $n } more.
