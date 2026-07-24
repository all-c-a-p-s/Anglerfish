# Anglerfish

Anglerfish is a WIP engine that plays a variant of poker called Heads-Up No-Limit Hold 'em.

Unlike my chess engine [Panda](https://github.com/all-c-a-p-s/Panda), which makes use of several heavily-researched techniques, Anglerfish is a project for which I did basically no research - I thought it would be fun to try and come up with an approach independently.

An extremely brief summary of its approach:

- uses a rough form of Bayesian filtering to assign probabilities to hands of the opponent, given the actions they've made
- uses an algorithm vaguely resembling MCTS to assign probabilities to actions in the future game tree

The name is a play on words from some poker slang: "shooting an angle" = bending the rules/cheating to gain an advantage (which my engine doesn't do). Instead, like a real anglerfish, my engine aims to trick and prey on "fish" (poker slang for bad players). It also loosely references Stockfish, though the big difference is that Stockfish is extremely good at the game it plays.
