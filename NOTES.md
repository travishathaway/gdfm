# Project Notes

## 2025-03-28

I'm currently trying to answer a couple different questions with this repository.
Here they are in no particular order:

- How quickly are maintainers responding to pull requests and issues?
- How long does it take to merge a pull request?
    - This should be broken down by pull request size measured as lines changed
- How long does it take to resolve an issue?
- How many issues are closed by the stale bot versus maintainers?
- How many active contributors are there?
    - What is an "active contributor"?

### Who are the maintainers?

One of the underlying things I need to figure out is who the maintainers
actually are. This can be a little tricky to figure out as it changes over
time. It might make the most sense to just stick with whoever currently has
merge rights for the repository and not worry about the history of the repository.

I think the best way to handle this in the application is to have users manually
add this information themselves. This will need to be a done on a per project
basis because each project can have a different list of maintainers.

In the application itself I think this would require something like this
to define:

```
gdfm init conda/conda --maintainers travishathaway,
```

## 2025-03-29

Making progress and learning how to use the octocrab API!

The first report I want to make with this tool would be for pull requests.
It would be nice to find the time to first review, and because we are aware
of the maintainers we could separate this number out for maintainers versus
non-maintainers.

For this, I'm going to need to determine when the pull request was made ready
for a review because pull requests can and often do begin as draft pull request.
So, to get a fair measurement of the turn around, I will need to measure from the
time it becomes available for a review.

Here's some code that code pilot gave to get started with it:

```rust
let events = octocrab
    .pulls(owner, repo)
    .list_timeline(pr_number)
    .send()
    .await?;

for event in events {
    if event.event == "ready_for_review" {
        println!("The pull request was switched to ready for review.");
    }
}
```

If a pull requets has been flipped back forth between draft and ready multiple
times, I should count each time as its own "review round". What that means is
I will need to measure the time from the event "ready for review" and find
the next review but only if it happened before the next event that switched
it back to draft status.

This part might get a little complicated, so I might initially just look at
the last "ready for review" event and only look at reviews that come after it.

### Other things to take into account

Another thing that would be cool to do is track pull request complexity
(commits, additions, deletions, changed_files). It would be cool to see
if higher complexity corresponds to longer review times.

### Author Association

I looked at the `author_association` field a little more and thankfully,
I think we'll be able to get away without having to manually track the
maintainers. This is what GitHub copilot told me about the fields:

```
The author_association field on a pull request in GitHub indicates the relationship
between the author of the pull request and the repository. GitHub determines this
field based on the user's role or association with the repository. Here are the
possible values and their meanings:

OWNER: The author is the owner of the repository.
COLLABORATOR: The author is a collaborator on the repository (has been explicitly granted write access).
MEMBER: The author is a member of the organization that owns the repository.
CONTRIBUTOR: The author has previously contributed to the repository (e.g., by having a merged pull request).
FIRST_TIMER: The author is contributing to the repository for the first time (this is their first contribution).
FIRST_TIME_CONTRIBUTOR: The author is contributing to the repository for the first time, but they have contributed to other repositories before.
NONE: The author has no association with the repository.
```


## 2025-03-30

Today I've been working a lot on the code that collects pull request. I wanted to learn how
to properly do concurrent request, so that took some time, but I think I've got it mostly
figure out now.

As far as making HTTP requests goes, the only thing I need to do is address the error case for
The code in `src/cli/collect.rs` is also super unorganized, so I'd like to clean that up a bit.

After I move on from that, I can start saving this stuff to the database which should be a lot
easier.

### later that day...

Ran into some bad news today. The GitHub rate limit is 5,000 requests per hour, so that means I
won't be able to make as many requests as I was hoping. This isn't too bad but I will need to
introduce some option that will let me restrict the size of my queries against the API.

The API calls that really get me are when it comes to fetching events and reviews for individual
pull requests. I have to perform these queries for each individual pull request which adds up
quick.
