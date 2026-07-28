module Game where

import Data.Bits
import Data.List (sortBy)
import Data.Ord (comparing)
import Data.Word
import Rng

data Suit
  = Hearts
  | Diamonds
  | Clubs
  | Spades
  deriving (Show, Eq, Enum, Bounded)

data Rank
  = Two
  | Three
  | Four
  | Five
  | Six
  | Seven
  | Eight
  | Nine
  | Ten
  | Jack
  | Queen
  | King
  | Ace
  deriving (Show, Eq, Enum, Bounded)

data Card = Card {suit :: Suit, rank :: Rank} deriving (Show, Eq)

cardCount :: Int
cardCount = 52

cards :: [Card]
cards =
  [ Card suit rank
  | suit <- [minBound .. maxBound],
    rank <- [minBound .. maxBound]
  ]

data Stage = Preflop | Flop | Turn | River deriving (Show, Eq)

data State = State
  { sbStack :: Int,
    sbThisStreet :: Int,
    bbStack :: Int,
    bbThisStreet :: Int,
    pot :: Int,
    deck :: [Card],
    turn :: Position,
    outcome :: Maybe HandOutcome,
    maxBet :: Int,
    stage :: Stage
  }
  deriving (Show)

sbHand :: State -> (Card, Card)
sbHand s =
  case deck s of
    x : y : _ -> (x, y)
    _ -> error "deck is corrupted"

bbHand :: State -> (Card, Card)
bbHand s =
  case deck s of
    _ : _ : x : y : _ -> (x, y)
    _ -> error "deck is corrupted"

flopCards :: State -> (Card, Card, Card)
flopCards s =
  case deck s of
    _ : _ : _ : _ : x : y : z : _ -> (x, y, z)
    _ -> error "deck is corrupted"

turnCard :: State -> Card
turnCard s =
  case deck s of
    _ : _ : _ : _ : _ : _ : _ : x : _ -> x
    _ -> error "deck is corrupted"

riverCard :: State -> Card
riverCard s =
  case deck s of
    _ : _ : _ : _ : _ : _ : _ : _ : x : _ -> x
    _ -> error "deck is corrupted"

suitName :: Suit -> String
suitName x =
  case x of
    Hearts -> "h"
    Diamonds -> "d"
    Clubs -> "c"
    Spades -> "s"

rankName :: Rank -> String
rankName x =
  case x of
    Two -> "2"
    Three -> "3"
    Four -> "4"
    Five -> "5"
    Six -> "6"
    Seven -> "7"
    Eight -> "8"
    Nine -> "9"
    Ten -> "T"
    Jack -> "J"
    Queen -> "Q"
    King -> "K"
    Ace -> "A"

cardName :: Card -> String
cardName x = rankName (rank x) ++ suitName (suit x)

handName :: (Card, Card) -> String
handName (x, y) = cardName x ++ cardName y

smallBlind :: Int
smallBlind = 1

bigBlind :: Int
bigBlind = 2

startingStack :: Int
startingStack = 100

data Position = SmallBlind | BigBlind deriving (Show, Eq)

-- Match = one set of hands until a player's stack reaches zero
data Match = Match {firstPos :: Position, state :: State}

data Outcome = Ongoing | FirstWins | SecondWins deriving (Show)

getOutcome :: Match -> Outcome
getOutcome x =
  case (sbStack (state x), bbStack (state x)) of
    (0, 0) -> error "both somehow have stack of zero after hand"
    (0, y) -> if (firstPos x) == SmallBlind then SecondWins else FirstWins
    (y, 0) -> if (firstPos x) == SmallBlind then FirstWins else SecondWins
    _ -> Ongoing

-- series of `matches_to_play` matches
data Series = Series {firstWins :: Int, secondWins :: Int, matchesToPlay :: Int}

data CardSet = CardSet {suitMasks :: [Word64], rankCounts :: [Int]}

aceHighStraight :: Word64
aceHighStraight = 0b_0001_1111_0000_0000

wheel :: Word64
wheel = 0b0001_0000_0000_1111

sfScore :: Int
sfScore = 2_000_000_000

quadsScore :: Int
quadsScore = 1_800_000_000

fhScore :: Int
fhScore = 1_600_000_000

flushScore :: Int
flushScore = 1_400_000_000

straightScore :: Int
straightScore = 1_200_000_000

threeOfKindScore :: Int
threeOfKindScore = 1_000_000_000

twoPairScore :: Int
twoPairScore = 800_000_000

pairScore :: Int
pairScore = 600_000_000

firstScore :: Int
firstScore = 10_000_000

secondScore :: Int
secondScore = 100_000

straightMask :: Int -> Word64
straightMask x =
  case x of
    9 -> wheel
    y -> shiftR aceHighStraight x

topN :: Word64 -> Int -> Word64
topN m n = if popCount m > n then topN (m .&. (m - 1)) n else m

rankMask :: Rank -> Word64
rankMask x =
  case x of
    Two -> 0b_0000_0000_0000_0001
    Three -> 0b_0000_0000_0000_0010
    Four -> 0b_0000_0000_0000_0100
    Five -> 0b_0000_0000_0000_1000
    Six -> 0b_0000_0000_0001_0000
    Seven -> 0b_0000_0000_0010_0000
    Eight -> 0b_0000_0000_0100_0000
    Nine -> 0b_0000_0000_1000_0000
    Ten -> 0b_0000_0001_0000_0000
    Jack -> 0b_0000_0010_0000_0000
    Queen -> 0b_0000_0100_0000_0000
    King -> 0b_0000_1000_0000_0000
    Ace -> 0b_0001_0000_0000_0000

straightHigh :: Word64 -> Maybe Int
straightHigh m =
  case help m 0 of
    Just x -> Just (10 - x)
    Nothing -> Nothing
  where
    help :: Word64 -> Int -> Maybe Int
    help m idx =
      case idx of
        10 -> Nothing
        x ->
          if m .&. straightMask x == straightMask x
            then Just x
            else help m (x + 1)

checkSf :: CardSet -> Maybe Int
checkSf x = help x 0
  where
    help :: CardSet -> Int -> Maybe Int
    help x idx =
      let masks = suitMasks x
       in case idx of
            4 -> Nothing
            y ->
              if popCount (masks !! y) >= 5
                then case straightHigh (masks !! y) of
                  Just z -> Just (sfScore + z)
                  Nothing -> help x (y + 1)
                else help x (y + 1)

checkFlush :: CardSet -> Maybe Int
checkFlush x = help x 0
  where
    help :: CardSet -> Int -> Maybe Int
    help x idx =
      let masks = suitMasks x
       in case idx of
            4 -> Nothing
            y ->
              if popCount (masks !! y) >= 5
                then Just (fromIntegral (topN (masks !! y) 5) + flushScore)
                else help x (y + 1)

checkStraight :: Word64 -> Maybe Int
checkStraight x = case straightHigh x of
  Just y -> Just (straightScore + y)
  Nothing -> Nothing

commonRanks :: CardSet -> ((Int, Int), (Int, Int))
commonRanks x =
  let indexedRankCounts = zip [0 .. 12] (rankCounts x)
      sorted = sortBy cmp indexedRankCounts
        where
          cmp (a1, b1) (a2, b2) =
            compare b2 b1 <> compare a2 a1
   in (sorted !! 0, sorted !! 1)

score :: CardSet -> Int
score x =
  let allSuits = foldr (.|.) 0 (suitMasks x)
   in case checkSf x of
        Just y -> y
        Nothing ->
          let ((top, topCount), (second, secondCount)) = commonRanks x
           in if topCount == 4
                then
                  let rest = allSuits .&. complement (shiftL 1 top)
                      kicker = topN rest 1
                   in quadsScore + firstScore * top + fromIntegral kicker
                else
                  if topCount == 3 && secondCount >= 2
                    then
                      fhScore + firstScore * top + secondScore * second
                    else case checkFlush x of
                      Just x -> x
                      Nothing -> case checkStraight allSuits of
                        Just x -> x
                        Nothing -> case (topCount, secondCount) of
                          (3, _) ->
                            let rest = allSuits .&. complement (shiftL 1 top)
                                kickers = topN rest 2
                             in threeOfKindScore + firstScore * top + fromIntegral kickers
                          (2, 2) ->
                            let rest = allSuits .&. complement ((shiftL 1 top) .|. (shiftL 1 second))
                                kicker = topN rest 1
                             in twoPairScore + firstScore * top + secondScore * second + fromIntegral kicker
                          (2, _) ->
                            let rest = allSuits .&. complement (shiftL 1 top)
                                kickers = topN rest 3
                             in pairScore + firstScore * top + fromIntegral kickers
                          _ ->
                            let kickers = topN allSuits 5
                             in fromIntegral kickers

blankCardSet :: CardSet
blankCardSet =
  let blankSuitMasks = replicate 4 0
      blankRankCounts = replicate 13 0
   in CardSet blankSuitMasks blankRankCounts

updatedCardSetWith :: CardSet -> Card -> CardSet
updatedCardSetWith cs card =
  CardSet
    { suitMasks =
        replace
          suitIdx
          (oldSuitMask .|. rankMask (rank card))
          (suitMasks cs),
      rankCounts =
        replace
          rankIdx
          (oldRankCount + 1)
          (rankCounts cs)
    }
  where
    suitIdx = fromEnum (suit card)
    rankIdx = fromEnum (rank card)

    oldSuitMask = suitMasks cs !! suitIdx
    oldRankCount = rankCounts cs !! rankIdx

setFromCards :: [Card] -> CardSet
setFromCards [] = blankCardSet
setFromCards (x : xs) = updatedCardSetWith (setFromCards xs) x

data HandOutcome = SBWins | BBWins | Draw deriving (Show, Eq)

adjudicateShowdown :: State -> HandOutcome
adjudicateShowdown x =
  let (sb1, sb2) = sbHand x
      (bb1, bb2) = bbHand x
      (a, b, c) = flopCards x
      d = turnCard x
      e = riverCard x
      sbSet = setFromCards [sb1, sb2, a, b, c, d, e]
      bbSet = setFromCards [bb1, bb2, a, b, c, d, e]
      sbScore = score sbSet
      bbScore = score bbSet
   in if sbScore > bbScore then SBWins else if sbScore == bbScore then Draw else BBWins
