## Core Concept

I want to build a speech to text application that runs on my machine. This application should allow users to click a hotkey on their keyboard, and then their microphone gets turned on, and then they can speak, and everything they type will be blazing fast, be ready to be pasted when they stop by clicking the same button or the hot key again.

This is going to be super helpful for AI agents so you can use voice commands and type stuff without needing to actually type it manually.

## MVP Requirements

- **Speed** — the most important thing here is this needs to be so freaking fast that it's unbelievable to the user's eye.
- **Fully local** — it should run completely locally on the user's machine, with no subscription payments whatsoever and no limits at all. Because it all runs locally — I know there are some AI models like Parakeet by Nvidia that run locally and allow this capability. But I need this feature to be blazing fast and fully local.
- **Grammatically correct** — the speech to text also obviously needs to be grammatically correct. That means it'll fix your punctuation and it'll fix your words so it is more accurate.
- **Multi-language** — it should use multiple languages, specifically Yoruba and most common languages like Spanish, Italian, French, Arabic,  English etc.
- **Local history** — all the users' history transcriptions should be stored on local machines as well. That way in case something fails, but it was recorded successfully, they can copy and paste it when they're recording at any time.
- **Escape/cancel behavior** — if they hit escape, it'll cancel that transcription with a short timer UI, and then that'll expire, and then that will cancel the transcription. Hitting escape twice will cancel deleting the transcription and will resume the transcription.
- Ability to listen in on the users speaker to capture sound from the computer, and a configurable hotkey i.e like granola

This is the MVP of this app.

## Technical Standpoint

- I don't know what are the best tech stacks for this. I know Tauri is the best for desktop applications 'cause it ships super light, and Rust backend probably gives you the best infrastructure. But since this is all local, it doesn't need user signups, etc. It's fully MIT and runs locally.
- The architecture of the app however has to be so freaking good — it has to be industry grade, production grade practice. Following similar concepts in YC.
- Since there's absolutely no paywall and everything is free, that does not need to be built, quite literally — but the source-of-truth resources / type elements might be needed.
- I don't want to use Swift for this app — Swift makes it very difficult to build apps.
- I don't want to send any data online — everything is local on the customer's device.
- There should be absolutely no failure opportunity for this app, because the architecture should be sound and rock solid. No spaghetti code, no billions of files. Just doing random stuff so that the app eventually ends up floating and drifting away — we don't want this. The code should be straightforward, MVP-focused, and should be following good coding practices.
- Need to build adapter layers for any sort of third-party tools — obviously for the AI specifically. That way in the future we can introduce different LLMs that users can use. So like for example, if we want to introduce user subscriptions to different tools or different models, like Apple's ASR or blazing fast multilingual models from Nvidia, etc. — those should also be possible. The user should be able to add that in by turning on those options.

## Dashboard / Metrics

- Should be a dashboard that shows them how much time they saved if they had actually typed all of these words.
- Total transcriptions ever done.
- Activity and real metrics that are worth looking at.
- An estimated words-per-minute, because now they're using voice — so it shows them a number to see this is working.

## UI / Design

- Super minimal glassmorphic UI design for everything. It should feel like it's an Apple product.
- This app's UI is not supposed to be a huge desktop application. It should sort of look like a widget almost — not permanently on the screen, obviously, but it can run the app itself in the background.
- Two main UI sections:
    1. The pill that shows that things are being transcribed.
    2. The UI of the desktop itself, where they can look at all the stuff mentioned above (dashboard/metrics).
- Regardless, both of these should have that same superglass morphic design, with frosted glass background.
- Settings page where they can change their hotkey, etc.

## Process / Deliverables

- You can create a folder inside documents/development in order to build this project.
- Before you proceed to build, I want you to generate an idea/ideation file that has the entire idea of my app and all the features I want in the app. Also some features that you think are good-to-have and must-have as well. Be a straightforward document, but don't beat around the bush.
- I also want a full plan of action from a technical standpoint as to how you're going to be building this app with the right tech stack. Remember, my focus is the app should be so good architecturally.
- Keep in mind: if I push some updates to this app, users should also be able to update their app to get all of this.
- You should not stop until the entire application is built to production. That means it's ready to go, and I can start using it reliably.
- Proceed: once you generate all of the plan documents that I asked for, and once I approve the plan, then you should fully build start to finish.
- Make sure you've mentioned the production-grade tech stack requirement.
- Everything should be absolutely free, zero resources from my end.