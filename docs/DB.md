# Merge time database

This design documents presents a crowdsouced database of merge times, for use in predictions.

## Prior art

Genlop can query [this db](https://gentoo.linuxhowtos.org/compiletimeestimator/) to get
estimates. But this solution has mostly fallen into disuse, and I can think of a few reasons :

* Entries are keyed by CPU model name
  - If your CPU is not in the DB, genlop won't return any result
  - There are many other variables that affect prediction time appart from CPU
* Database is updated manually, by sending the output of some commands to the website admins
  - It's a cumbersome procedure, both for users and admins
  - Judging by the current list of CPU models, no new data has been accepted for years
  - It's necessarily noisy (single data point)
  - It AFAICT doesn't get updates (but packages do get slower/faster to compile over time)
  - I have some data privacy concerns

## New design

### Matching algorithm

Instead of storing data keyed by CPU model, we store many samples (list of merge times from one
machine) without a predefined key. When a client wants a prediction, it sends a sample of its local
merge times which the server matches against the DB samples. Each sample gets a match rating, and we
look for the requested prediction in the best-matched samples.

Binary merges should be kept out of the database. USE flags might be worth recording, if we have
enough data and a good enough matching algorithm to use it.

Pros:
* This is more resilient to different setups (CPU, memory, USE, etc)
* It can ponderate the estimate from many samples, and give a confidence ranking

Cons:
* User has to upload some merge time data, not suitable very early in the install proces
* Algorithm is more CPU- and memory-intensive

### Database update, and privacy

Sending new samples should be easy and automated, and not hindered by privacy concerns.

* Samples must be anonymous
  - No user id needed or recorded
  - Samples are disjoint (by remembering the last uploaded merge client-side), to avoid following a
    user across samples
  - Minimal information: just ebuild+version+use+time, and a global date for the sample
  - Minimize server logs to what is needed for abuse prevention
* Webservice ingests new samples sent by any user
  - Public API is append-only (no CRUD)
  - Old data get purged automatically
  - Client-side crontab sets reasonable defaults (frequency, and min/max size)
  - Same webservice also serves prediction queries
  - Should be usable by the whole community, not just emlop users
* Upload must be opt-in
  - User can review and filter data before upload
  - Ebuild can enable crontab if a USE flag is set
* Clear, maximally-open data license
* Trustable code
  - Deploy from github CI
  - Easy to run a private instance

## Implementation choices
