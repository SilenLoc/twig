# Product: Namespace invitations

## Problem

As an administrator, I want to invite people to the right namespace by email, with the right level of access and an expiry, so they can create their own account and get to work without me creating their account for them.

## Success metric

At least 90% of unexpired invitations are accepted and result in a completed account setup, measured as completed signups divided by sent invitations over a rolling 30-day period.

## Announcement — the blog post before the feature

You can now invite people to join a namespace by email. Choose the namespace, decide whether they should be an owner or contributor, and set how long the link stays valid. Contributors can work with repositories in their namespace without being able to invite others or create namespaces and repositories; they also cannot force-push to main. Invitees finish setup themselves by choosing a username and password, with their email already filled in.

## Screens

- `./mockups/user-management-invites.html` — User Management with Users and Invites tabs, invite form, and invitation list.
- `./mockups/accept-invitation.html` — invitation signup form with the invited email prefilled.
