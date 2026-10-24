use crate::memory::message::messages::{Message, MessageType};
use cambc::ChampionType;

const BOARD_BYTES: usize = 8;
const FREE_BYTE: u8 = 0xFF;
const FREE_PREFIX: u8 = 0b11_1111;
const MAX_MESSAGE_BYTES: usize = 4;
const MINIMUM_SYNC_FUEL: u32 = 15_000;

fn is_free_byte(byte: u8) -> bool {
    byte >> 2 == FREE_PREFIX
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageSpan {
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeenMessage {
    pub offset: usize,
    pub length: usize,
    pub bytes: [u8; MAX_MESSAGE_BYTES],
}

#[derive(Debug, Clone, PartialEq)]
pub struct OwnedMessage {
    pub span: MessageSpan,
    pub bytes: [u8; MAX_MESSAGE_BYTES],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReceivedMessage {
    pub span: MessageSpan,
    pub message: Message,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingResponse {
    pub span: MessageSpan,
    pub request_key: (MessageType, u8),
    pub responder: ChampionType,
}

struct DirtyRange {
    start: usize,
    end: usize,
}

impl DirtyRange {
    fn new() -> Self {
        Self {
            start: BOARD_BYTES,
            end: 0,
        }
    }

    fn mark(&mut self, offset: usize, length: usize) {
        self.start = self.start.min(offset);
        self.end = self.end.max(offset + length);
    }

    fn range(&self) -> Option<(usize, usize)> {
        if self.start < self.end {
            Some((self.start, self.end))
        } else {
            None
        }
    }
}

fn scan_board(shared: &[u8; BOARD_BYTES]) -> (Vec<MessageSpan>, [bool; BOARD_BYTES]) {
    let mut spans: Vec<MessageSpan> = Vec::new();
    let mut occupied: [bool; BOARD_BYTES] = [false; BOARD_BYTES];
    let mut position: usize = 0;

    while position < BOARD_BYTES {
        let byte: u8 = shared[position];
        if is_free_byte(byte) {
            position += 1;
            continue;
        }

        let length: usize = MessageType::from_nibble(byte >> 4).byte_length();
        let end: usize = (position + length).min(BOARD_BYTES);
        for flag in &mut occupied[position..end] {
            *flag = true;
        }
        if position + length <= BOARD_BYTES {
            spans.push(MessageSpan {
                offset: position,
                length,
            });
        }
        position = end;
    }

    (spans, occupied)
}

fn find_free_run(
    occupied: &[bool; BOARD_BYTES],
    length: usize,
    search_start: usize,
) -> Option<usize> {
    for step in 0..BOARD_BYTES {
        let offset: usize = (search_start + step) % BOARD_BYTES;
        if offset + length > BOARD_BYTES {
            continue;
        }
        if occupied[offset..offset + length].iter().all(|flag| !*flag) {
            return Some(offset);
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageBoard {
    pub owned_messages: Vec<OwnedMessage>,
    pub pending_messages: Vec<Message>,
    pub pending_responses: Vec<PendingResponse>,

    pub inbox: Vec<ReceivedMessage>,

    pub responses: Vec<Message>,
    pub seen_messages: Vec<SeenMessage>,
    pub next_fit_start: usize,
}

impl Default for MessageBoard {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageBoard {
    pub fn new() -> Self {
        Self {
            owned_messages: Vec::new(),
            pending_messages: Vec::new(),
            pending_responses: Vec::new(),
            inbox: Vec::new(),
            responses: Vec::new(),
            seen_messages: Vec::new(),
            next_fit_start: 0,
        }
    }

    pub fn send(&mut self, message: Message) {
        self.pending_messages.push(message);
    }

    pub fn respond_to(&mut self, received: &ReceivedMessage, responder: ChampionType) -> bool {
        match received.message.request_key() {
            Some(request_key) => {
                self.pending_responses.push(PendingResponse {
                    span: received.span,
                    request_key,
                    responder,
                });
                true
            }
            None => false,
        }
    }

    pub fn sync(&mut self) {
        self.exchange(true);
    }

    pub fn flush(&mut self) {
        if self.pending_messages.is_empty() && self.pending_responses.is_empty() {
            return;
        }
        self.exchange(false);
    }

    fn exchange(&mut self, start_of_turn: bool) {
        if cambc::fuel_remaining().is_ok_and(|fuel| fuel < MINIMUM_SYNC_FUEL as _) {
            return;
        }

        let mut shared: [u8; BOARD_BYTES] = [0; BOARD_BYTES];
        if cambc::shared_memory().read(0, &mut shared).is_err() {
            return;
        }

        let mut dirty: DirtyRange = DirtyRange::new();
        if shared == [0; BOARD_BYTES] {
            shared = [FREE_BYTE; BOARD_BYTES];
            dirty.mark(0, BOARD_BYTES);
            self.owned_messages.clear();
            self.seen_messages.clear();
        }

        let mut replies: Vec<Message> = Vec::new();
        let mut new_owned: Vec<OwnedMessage> = Vec::new();
        if start_of_turn {
            for owned in &self.owned_messages {
                let span: MessageSpan = owned.span;
                let current: &[u8] = &shared[span.offset..span.offset + span.length];
                if current != &owned.bytes[..span.length] {
                    replies.push(Message::from_bytes(current));
                }
                for byte in &mut shared[span.offset..span.offset + span.length] {
                    *byte = FREE_BYTE;
                }
                dirty.mark(span.offset, span.length);
            }
        } else {
            new_owned = self.owned_messages.clone();
        }

        let (spans, mut occupied): (Vec<MessageSpan>, [bool; BOARD_BYTES]) = scan_board(&shared);

        let mut received: Vec<ReceivedMessage> = Vec::new();
        let mut seen_next: Vec<SeenMessage> = Vec::new();
        if start_of_turn {
            for span in spans {
                let mut bytes: [u8; MAX_MESSAGE_BYTES] = [0; MAX_MESSAGE_BYTES];
                bytes[..span.length]
                    .copy_from_slice(&shared[span.offset..span.offset + span.length]);
                let seen: SeenMessage = SeenMessage {
                    offset: span.offset,
                    length: span.length,
                    bytes,
                };
                if !self.seen_messages.contains(&seen) {
                    received.push(ReceivedMessage {
                        span,
                        message: Message::from_bytes(&bytes[..span.length]),
                    });
                }
                seen_next.push(seen);
            }
        } else {
            seen_next = self.seen_messages.clone();
        }

        for response in &self.pending_responses {
            let span: MessageSpan = response.span;
            if span.offset + span.length > BOARD_BYTES || is_free_byte(shared[span.offset]) {
                continue;
            }

            let mut message: Message =
                Message::from_bytes(&shared[span.offset..span.offset + span.length]);
            if message.request_key() != Some(response.request_key) {
                continue;
            }
            if !message.respond(response.responder) {
                continue;
            }

            let updated: Vec<u8> = message.as_bytes();
            if updated.len() != span.length {
                continue;
            }
            shared[span.offset..span.offset + span.length].copy_from_slice(&updated);
            dirty.mark(span.offset, span.length);

            if let Some(seen) = seen_next
                .iter_mut()
                .find(|seen| seen.offset == span.offset && seen.length == span.length)
            {
                seen.bytes[..span.length].copy_from_slice(&updated);
            }
        }

        let mut unplaced: Vec<Message> = Vec::new();
        let mut next_fit_start: usize = self.next_fit_start;
        for message in &self.pending_messages {
            let message_bytes: Vec<u8> = message.as_bytes();
            if message_bytes.is_empty() || is_free_byte(message_bytes[0]) {
                continue;
            }

            match find_free_run(&occupied, message_bytes.len(), next_fit_start) {
                Some(offset) => {
                    let length: usize = message_bytes.len();
                    shared[offset..offset + length].copy_from_slice(&message_bytes);
                    for flag in &mut occupied[offset..offset + length] {
                        *flag = true;
                    }
                    let mut bytes: [u8; MAX_MESSAGE_BYTES] = [0; MAX_MESSAGE_BYTES];
                    bytes[..length].copy_from_slice(&message_bytes);
                    new_owned.push(OwnedMessage {
                        span: MessageSpan { offset, length },
                        bytes,
                    });
                    dirty.mark(offset, length);
                    next_fit_start = (offset + length) % BOARD_BYTES;
                }
                None => unplaced.push(message.clone()),
            }
        }

        if let Some((start, end)) = dirty.range() {
            if cambc::shared_memory()
                .write(start, &shared[start..end])
                .is_err()
            {
                return;
            }
        }

        self.owned_messages = new_owned;
        self.pending_messages = unplaced;
        self.pending_responses.clear();
        self.seen_messages = seen_next;
        self.next_fit_start = next_fit_start;
        self.inbox.extend(received);
        self.responses.extend(replies);
    }
}
