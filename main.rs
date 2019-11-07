use rand::Rng;
use std::cmp;
use tcod::colors::*;
use tcod::console::*;
use tcod::input::Key;
use tcod::input::KeyCode::*;

use tcod::map::{FovAlgorithm, Map as FovMap};

#[derive(PartialEq, Eq, Clone, Copy)]
enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const FPS: i32 = 20;

const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 45;

const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 40;

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Restrictive;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

const MAX_ROOM_MONSTERS: i32 = 3;

const PLAYER: usize = 0;

const DARK_WALL: Color = Color {
    r: 0_0,
    g: 0_0,
    b: 100,
};
const LIGHT_WALL: Color = Color {
    r: 130,
    g: 110,
    b: 50,
};
const DARK_GROUND: Color = Color {
    r: 50,
    g: 50,
    b: 150,
};
const LIGHT_GROUND: Color = Color {
    r: 200,
    g: 180,
    b: 50,
};

fn move_towards(id: usize, targetx: i32, targety: i32, map: &Map, objects: &mut [Object]) {
    let dx = targetx - objects[id].x;
    let dy = targety - objects[id].y;
    let distance = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();

    let dx = (dx as f32 / distance).round() as i32;
    let dy = (dy as f32 / distance).round() as i32;
    move_by(id, dx, dy, map, objects);
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y + h,
        }
    }

    fn center(&self) -> (i32, i32) {
        let centerx = (self.x1 + self.x2) / 2;
        let centery = (self.y1 + self.y2) / 2;
        (centerx, centery)
    }

    fn intersect(&self, other: &Rect) -> bool {
        (self.x1 <= other.x2)
            && (self.x2 >= other.x1)
            && (self.y1 <= other.y2)
            && (self.y2 >= other.y1)
    }
}

fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    if map[x as usize][y as usize].blocked {
        return true;
    }

    objects
        .iter()
        .any(|o: &Object| o.blocks && o.pos() == (x, y))
}

fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    let (x, y) = objects[id].pos();
    if !is_blocked(x + dx, y + dy, map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}

fn player_move_or_attack(dx: i32, dy: i32, game: &Game, objects: &mut [Object]) {
    let x = objects[PLAYER].x + dx;
    let y = objects[PLAYER].y + dy;

    let id = objects.iter().position(|o: &Object| o.pos() == (x, y));

    match id {
        Some(target) => {
            println!("{} laughs at you!", objects[target].name);
        }
        None => {
            move_by(PLAYER, dx, dy, &game.map, objects);
        }
    }
}

struct Tcod {
    root: Root,
    con: Offscreen,
    fov: FovMap,
}

struct Fighter {
    max_hp: i32,
    hp: i32,
    defense: i32,
    power: i32,
}

enum Ai {
    Basic,
}

struct Object {
    x: i32,
    y: i32,
    char: char,
    color: Color,
    name: String,
    blocks: bool,
    alive: bool,
    fighter: Option<Fighter>,
    ai: Option<Ai>,
}

impl Object {
    fn new(x: i32, y: i32, char: char, name: &str, color: Color, blocks: bool) -> Object {
        Object {
            x,
            y,
            char,
            color,
            name: name.into(),
            blocks,
            alive: false,
            fighter: None,
            ai: None,
        }
    }

    fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    fn distance_to(&self, other: &Object) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        ((dx.pow(2) + dy.pow(2)) as f32).sqrt()
    }

    fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    fn draw(&self, con: &mut dyn Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }
}
#[derive(Clone, Copy, Debug)]
struct Tile {
    blocked: bool,
    block_sight: bool,
    explored: bool,
}

impl Tile {
    fn empty() -> Self {
        Tile {
            blocked: false,
            block_sight: false,
            explored: false,
        }
    }

    fn wall() -> Self {
        Tile {
            blocked: true,
            block_sight: true,
            explored: false,
        }
    }
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>) {
    let num_mon = rand::thread_rng().gen_range(0, MAX_ROOM_MONSTERS + 1);

    for _ in 0..num_mon {
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        let mut monster = if rand::random::<f32>() < 0.8 {
            let mut orc = Object::new(x, y, 'o', "Ogre", DESATURATED_GREEN, true);
            orc.fighter = Some(Fighter {
                max_hp: 10,
                hp: 10,
                defense: 0,
                power: 3,
            });
            orc.ai = Some(Ai::Basic);
            orc
        } else {
            let mut troll = Object::new(x, y, 'T', "Troll", DARKER_GREEN, true);
            troll.fighter = Some(Fighter {
                max_hp: 16,
                hp: 16,
                defense: 1,
                power: 4,
            });
            troll.ai = Some(Ai::Basic);
            troll
        };

        if !is_blocked(x, y, map, objects) {
            monster.alive = true;
            objects.push(monster);
        }
    }
}

type Map = Vec<Vec<Tile>>;

struct Game {
    map: Map,
}

fn make_map(objects: &mut Vec<Object>) -> Map {
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];

    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let x = rand::thread_rng().gen_range(0, MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, MAP_HEIGHT - h);

        let new_room = Rect::new(x, y, w, h);
        let failed = rooms.iter().any(|other| new_room.intersect(other));

        if !failed {
            create_room(new_room, &mut map);
            place_objects(new_room, &map, objects);
            let (nx, ny) = new_room.center();

            if rooms.is_empty() {
                objects[PLAYER].set_pos(nx, ny);
            } else {
                let (prevx, prevy) = rooms[rooms.len() - 1].center();

                if rand::random() {
                    h_tunnel(prevx, nx, prevy, &mut map);
                    v_tunnel(prevy, ny, nx, &mut map);
                } else {
                    v_tunnel(prevy, ny, prevx, &mut map);
                    h_tunnel(prevx, nx, ny, &mut map);
                }
            }

            rooms.push(new_room);
        }
    }

    // let room1 = Rect::new(20, 15, 10, 15);
    // let room2 = Rect::new(50, 15, 10, 15);

    // create_room(room1, &mut map);
    // create_room(room2, &mut map);

    map
}

fn h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..cmp::max(x1, x2) + 1 {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn render_all(tcod: &mut Tcod, game: &mut Game, objects: &[Object], recompute: bool) {
    if recompute {
        let p = &objects[PLAYER];
        tcod.fov
            .compute_fov(p.x, p.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }

    for o in objects {
        if tcod.fov.is_in_fov(o.x, o.y) {
            o.draw(&mut tcod.con);
        }
    }

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let visible = tcod.fov.is_in_fov(x, y);
            let wall = game.map[x as usize][y as usize].block_sight;
            let color = match (visible, wall) {
                (false, true) => DARK_WALL,
                (false, false) => DARK_GROUND,
                (true, true) => LIGHT_WALL,
                (true, false) => LIGHT_GROUND,
            };
            let explored = &mut game.map[x as usize][y as usize].explored;
            if visible {
                *explored = true;
            }
            if *explored {
                tcod.con
                    .set_char_background(x, y, color, BackgroundFlag::Set);
            }
        }
    }

    blit(
        &tcod.con,
        (0, 0),
        (SCREEN_WIDTH, SCREEN_HEIGHT),
        &mut tcod.root,
        (0, 0),
        1.0,
        1.0,
    );
}

fn input(tcod: &mut Tcod, game: &Game, objects: &mut [Object]) -> PlayerAction {
    let key = tcod.root.wait_for_keypress(true);
    let alive = objects[PLAYER].alive;

    match (key, key.text(), alive) {
        (Key { code: Up, .. }, _, true) => {
            move_by(PLAYER, 0, -1, &game.map, objects);
            return PlayerAction::TookTurn;
        }
        (Key { code: Down, .. }, _, true) => {
            move_by(PLAYER, 0, 1, &game.map, objects);
            return PlayerAction::TookTurn;
        }
        (Key { code: Left, .. }, _, true) => {
            move_by(PLAYER, -1, 0, &game.map, objects);
            return PlayerAction::TookTurn;
        }
        (Key { code: Right, .. }, _, true) => {
            move_by(PLAYER, 1, 0, &game.map, objects);
            return PlayerAction::TookTurn;
        }
        (Key { code: Escape, .. }, _, _) => PlayerAction::Exit,
        (
            Key {
                code: Enter,
                alt: true,
                ..
            },
            _,
            _,
        ) => {
            let fs = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fs);
            return PlayerAction::DidntTakeTurn;
        }
        _ => PlayerAction::DidntTakeTurn,
    };

    PlayerAction::DidntTakeTurn
}

fn main() {
    let mut player = Object::new(0, 0, '@', "Quincy", WHITE, true);
    player.alive = true;
    player.fighter = Some(Fighter {
        max_hp: 30,
        hp: 30,
        defense: 2,
        power: 5,
    });

    let mut objects = vec![player];

    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("QRPG")
        .init();

    let mut game = Game {
        map: make_map(&mut objects),
    };

    // h_tunnel(25, 55, 25, &mut game.map);
    // h_tunnel(25, 55, 26, &mut game.map);

    tcod::system::set_fps(FPS);

    let mut tcod = Tcod {
        root,
        con: Offscreen::new(MAP_WIDTH, MAP_HEIGHT),
        fov: FovMap::new(MAP_WIDTH, MAP_HEIGHT),
    };

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            tcod.fov.set(
                x,
                y,
                !game.map[x as usize][y as usize].block_sight,
                !game.map[x as usize][y as usize].blocked,
            );
        }
    }

    let mut prev_player_pos = (-1, -1);

    while !tcod.root.window_closed() {
        tcod.con.clear();
        let recompute = prev_player_pos != (objects[PLAYER].pos());
        render_all(&mut tcod, &mut game, &objects, recompute);
        tcod.root.flush();
        prev_player_pos = (objects[PLAYER].x, objects[PLAYER].y);
        let exit = input(&mut tcod, &game, &mut objects);
        if exit == PlayerAction::Exit {
            break;
        }
        if objects[PLAYER].alive && exit != PlayerAction::DidntTakeTurn {
            for o in &objects {
                if (o as *const Object) != (&objects[PLAYER] as *const Object) {
                    // println!("The {} growls!", o.name);
                }
            }
        }
    }
}
