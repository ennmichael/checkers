use crate::checkers::{CheckersGame, JumpResult, Table, Team};
use actix::prelude::*;
use rand;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::mem;

#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PlayerID(pub usize);

#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct GameID(pub usize);

struct Matchmaker {
    enqueued: Option<Matchup>,
    last_match_id: GameID,
}

impl Matchmaker {
    fn new() -> Self {
        Self {
            enqueued: None,
            last_match_id: GameID(0),
        }
    }

    fn matchup(&mut self, matchup: Matchup) -> Option<(Matchup, Matchup, GameID)> {
        match self.enqueued.take() {
            Some(enqueued_matchup) => {
                self.last_match_id.0 += 1;

                let mut light = matchup;
                let mut dark = enqueued_matchup;
                if rand::random() {
                    mem::swap(&mut light, &mut dark);
                }

                Some((light, dark, GameID(self.last_match_id.0 - 1)))
            }
            None => {
                self.enqueued = Some(matchup);
                None
            }
        }
    }
}

pub struct GameMaster {
    matchmaker: Matchmaker,
    games: HashMap<GameID, OngoingGame>,
    players_in_games: HashMap<PlayerID, GameID>,
}

struct OngoingGame {
    game: CheckersGame,
    light_player: Player,
    dark_player: Player,
}

struct Player {
    id: PlayerID,
    game_state_recipient: Recipient<GameState>,
    game_update_recipient: Recipient<GameUpdate>,
    bad_jump_recipient: Recipient<BadJump>,
}

impl OngoingGame {
    fn jump(&mut self, player_id: PlayerID, from: usize, to: usize) {
        if self.is_on_turn(player_id) {
            match self.game.jump(from, to) {
                JumpResult::Good {
                    captured_piece,
                    crowned,
                } => self.send_updates(from, to, captured_piece, crowned),
                JumpResult::Bad => self.send_bad_jump(player_id),
            }
        } else {
            self.send_bad_jump(player_id)
        }
    }

    fn is_on_turn(&self, player_id: PlayerID) -> bool {
        match self.team(player_id) {
            Some(team) => team == self.game.team_on_turn(),
            None => false,
        }
    }

    fn team(&self, player_id: PlayerID) -> Option<Team> {
        if self.light_player.id == player_id {
            Some(Team::Light)
        } else if self.dark_player.id == player_id {
            Some(Team::Dark)
        } else {
            None
        }
    }

    fn bad_jump_recipient(&self, player_id: PlayerID) -> Option<&Recipient<BadJump>> {
        if self.light_player.id == player_id {
            Some(&self.light_player.bad_jump_recipient)
        } else if self.dark_player.id == player_id {
            Some(&self.dark_player.bad_jump_recipient)
        } else {
            None
        }
    }

    fn send_game_state(&self) {
        let msg = GameState {
            table: *self.game.table(),
            team_on_turn: self.game.team_on_turn(),
            winner: self.game.winner(),
        };
        self.light_player
            .game_state_recipient
            .do_send(msg.clone())
            .unwrap();
        self.dark_player.game_state_recipient.do_send(msg).unwrap();
    }

    fn send_updates(&self, from: usize, to: usize, captured_piece: Option<usize>, crowned: bool) {
        let msg = GameUpdate {
            from,
            to,
            crowned,
            captured_piece,
            team_on_turn: self.game.team_on_turn(),
            winner: self.game.winner(),
        };

        self.light_player
            .game_update_recipient
            .do_send(msg.clone())
            .unwrap();
        self.dark_player.game_update_recipient.do_send(msg).unwrap();
    }

    fn send_bad_jump(&self, player_id: PlayerID) {
        if let Some(r) = self.bad_jump_recipient(player_id) {
            r.do_send(BadJump).unwrap();
        }
    }
}

pub type GameMasterAddr = Addr<GameMaster>;

#[derive(Message)]
#[rtype(result = "()")]
pub struct Matchup {
    pub game_found_recipient: Recipient<GameFound>,
    pub game_update_recipient: Recipient<GameUpdate>,
    pub game_state_recipient: Recipient<GameState>,
    pub bad_jump_recipient: Recipient<BadJump>,
    pub player_id: PlayerID,
}

#[derive(Message, Serialize)]
#[rtype(result = "()")]
pub struct GameFound {
    game_id: GameID,
    light_player: PlayerID,
    dark_player: PlayerID,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Jump {
    pub player_id: PlayerID,
    pub from: usize,
    pub to: usize,
}

#[derive(Message, Clone, Serialize)]
#[rtype(result = "()")]
pub struct GameState {
    table: Table,
    team_on_turn: Team,
    winner: Option<Team>,
}

#[derive(Message, Clone, Serialize)]
#[rtype(result = "()")]
pub struct GameUpdate {
    from: usize,
    to: usize,
    crowned: bool,
    captured_piece: Option<usize>,
    team_on_turn: Team,
    winner: Option<Team>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BadJump;

impl GameMaster {
    pub fn new() -> Self {
        Self {
            matchmaker: Matchmaker::new(),
            games: HashMap::new(),
            players_in_games: HashMap::new(),
        }
    }
}

impl Actor for GameMaster {
    type Context = Context<Self>;
}

impl Handler<Matchup> for GameMaster {
    type Result = ();

    fn handle(&mut self, msg: Matchup, _: &mut Self::Context) {
        if let Some((light, dark, game_id)) = self.matchmaker.matchup(msg) {
            let game = OngoingGame {
                light_player: Player {
                    id: light.player_id,
                    game_state_recipient: light.game_state_recipient,
                    game_update_recipient: light.game_update_recipient,
                    bad_jump_recipient: light.bad_jump_recipient,
                },
                dark_player: Player {
                    id: dark.player_id,
                    game_state_recipient: dark.game_state_recipient,
                    game_update_recipient: dark.game_update_recipient,
                    bad_jump_recipient: dark.bad_jump_recipient,
                },
                game: CheckersGame::new(),
            };

            game.send_game_state();

            self.games.insert(game_id, game);
            self.players_in_games.insert(light.player_id, game_id);
            self.players_in_games.insert(dark.player_id, game_id);

            light
                .game_found_recipient
                .do_send(GameFound {
                    game_id,
                    light_player: light.player_id,
                    dark_player: dark.player_id,
                })
                .unwrap();

            dark.game_found_recipient
                .do_send(GameFound {
                    game_id,
                    light_player: light.player_id,
                    dark_player: dark.player_id,
                })
                .unwrap();
        }
    }
}

impl Handler<Jump> for GameMaster {
    type Result = ();

    fn handle(&mut self, msg: Jump, _: &mut Self::Context) -> Self::Result {
        match self.players_in_games.get(&msg.player_id) {
            Some(game_id) => {
                let game = self
                    .games
                    .get_mut(game_id)
                    .expect("player was in a non-existent game");

                game.jump(msg.player_id, msg.from, msg.to);
            }
            None => unimplemented!(),
        }
    }
}
