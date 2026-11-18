// Run with: npm test   (node's built-in runner; no dependencies)
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseReminder } from './parse.ts';

// Thursday 8 October 2026, 10:30 local time.
const NOW = new Date(2026, 9, 8, 10, 30, 0, 0);
const at = (y: number, mo: number, d: number, h = 0, mi = 0) => new Date(y, mo - 1, d, h, mi).getTime();
const parse = (text: string) => parseReminder(text, NOW);

describe('relative times', () => {
  it('minutes', () => {
    const p = parse('stretch in 20 min');
    assert.equal(p.title, 'Stretch');
    assert.equal(p.dueAt, at(2026, 10, 8, 10, 50));
  });
  it('hours, phrase first', () => {
    const p = parse('in 2 hours buy milk');
    assert.equal(p.title, 'Buy milk');
    assert.equal(p.dueAt, at(2026, 10, 8, 12, 30));
  });
  it('"an hour" and "half an hour"', () => {
    assert.equal(parse('check oven in an hour').dueAt, at(2026, 10, 8, 11, 30));
    assert.equal(parse('stretch in half an hour').dueAt, at(2026, 10, 8, 11, 0));
  });
  it('compound durations', () => {
    assert.equal(parse('in 1 hour 30 minutes check oven').dueAt, at(2026, 10, 8, 12, 0));
  });
  it('number words and "from now"', () => {
    assert.equal(parse('in twenty minutes call back').dueAt, at(2026, 10, 8, 10, 50));
    assert.equal(parse('call back 15 minutes from now').dueAt, at(2026, 10, 8, 10, 45));
  });
  it('days keep the current time of day', () => {
    const p = parse('in 3 days send invoice');
    assert.equal(p.title, 'Send invoice');
    assert.equal(p.dueAt, at(2026, 10, 11, 10, 30));
  });
});

describe('days and clock times', () => {
  it('tomorrow at 5pm', () => {
    const p = parse('call mom tomorrow at 5pm');
    assert.equal(p.title, 'Call mom');
    assert.equal(p.dueAt, at(2026, 10, 9, 17, 0));
  });
  it('time before the day', () => {
    assert.equal(parse('5:30pm tomorrow call mom').dueAt, at(2026, 10, 9, 17, 30));
  });
  it('a weekday name', () => {
    const p = parse('dentist friday 3pm');
    assert.equal(p.title, 'Dentist');
    assert.equal(p.dueAt, at(2026, 10, 9, 15, 0));
  });
  it('a bare weekday defaults to 9:00', () => {
    assert.equal(parse('monday standup').dueAt, at(2026, 10, 12, 9, 0));
  });
  it('the same weekday means next week once today is underway', () => {
    assert.equal(parse('thursday review').dueAt, at(2026, 10, 15, 9, 0));
  });
  it('a date', () => {
    assert.equal(parse('pay rent nov 1').dueAt, at(2026, 11, 1, 9, 0));
    assert.equal(parse('1 nov pay rent').dueAt, at(2026, 11, 1, 9, 0));
    assert.equal(parse('Oct 12 5pm').dueAt, at(2026, 10, 12, 17, 0));
  });
  it('a date that has already passed this year means next year', () => {
    assert.equal(parse('birthday jan 5').dueAt, at(2027, 1, 5, 9, 0));
  });
  it('an ISO date with a 24-hour time', () => {
    const p = parse('2026-12-25 15:00 gift');
    assert.equal(p.title, 'Gift');
    assert.equal(p.dueAt, at(2026, 12, 25, 15, 0));
  });
  it('tonight, morning, noon, o\'clock', () => {
    assert.equal(parse('tonight call sam').dueAt, at(2026, 10, 8, 20, 0));
    assert.equal(parse('tomorrow morning run').dueAt, at(2026, 10, 9, 9, 0));
    assert.equal(parse('lunch with sam at noon').dueAt, at(2026, 10, 8, 12, 0));
    assert.equal(parse('call at 5 o\'clock').dueAt, at(2026, 10, 8, 17, 0));
  });
  it('next week', () => {
    assert.equal(parse('review next week').dueAt, at(2026, 10, 15, 9, 0));
  });
});

describe('ambiguous and past times', () => {
  it('a bare 1–6 means the afternoon', () => {
    assert.equal(parse('gym at 6').dueAt, at(2026, 10, 8, 18, 0));
  });
  it('a bare hour that has passed this morning means this evening', () => {
    assert.equal(parse('meeting at 9').dueAt, at(2026, 10, 8, 21, 0));
  });
  it('a time that already passed today rolls to tomorrow', () => {
    assert.equal(parse('call dad at 8am').dueAt, at(2026, 10, 9, 8, 0));
  });
  it('24-hour times are taken literally', () => {
    assert.equal(parse('meet at 17:30').dueAt, at(2026, 10, 8, 17, 30));
    assert.equal(parse('train 07:15').dueAt, at(2026, 10, 9, 7, 15));
  });
  it('"today" with no time picks a sensible hour from now', () => {
    const due = parse('call bank today').dueAt!;
    assert.ok(due > NOW.getTime() && due <= NOW.getTime() + 3_900_000, 'about an hour ahead');
  });
});

describe('repeats', () => {
  it('every weekday', () => {
    const p = parse('standup every weekday at 9:30');
    assert.equal(p.title, 'Standup');
    assert.equal(p.repeat, 'weekdays');
    assert.equal(p.dueAt, at(2026, 10, 9, 9, 30));
  });
  it('weekday repeats skip the weekend', () => {
    // 9 Oct 2026 is a Friday; "9:30" has passed for a 10:30 start on Friday? No: it starts Friday, still ahead.
    const p = parseReminder('standup every weekday 9:30', new Date(2026, 9, 9, 11, 0));
    assert.equal(p.dueAt, at(2026, 10, 12, 9, 30), 'Friday 11:00 → next is Monday');
  });
  it('every N minutes starts one interval from now', () => {
    const p = parse('every 45 min drink water');
    assert.equal(p.title, 'Drink water');
    assert.equal(p.repeat, 'interval:45');
    assert.equal(p.dueAt, at(2026, 10, 8, 11, 15));
  });
  it('every N hours, hourly', () => {
    assert.equal(parse('every 2 hours stretch').repeat, 'interval:120');
    assert.equal(parse('hourly stretch').repeat, 'interval:60');
  });
  it('every day at a time', () => {
    const p = parse('remind me to take pills every day at 8pm');
    assert.equal(p.title, 'Take pills');
    assert.equal(p.repeat, 'daily');
    assert.equal(p.dueAt, at(2026, 10, 8, 20, 0));
  });
  it('every <weekday>', () => {
    const p = parse('every monday 10am review');
    assert.equal(p.title, 'Review');
    assert.equal(p.repeat, 'weekly');
    assert.equal(p.dueAt, at(2026, 10, 12, 10, 0));
  });
  it('weekly and monthly', () => {
    assert.equal(parse('backup weekly').repeat, 'weekly');
    assert.equal(parse('pay rent monthly').repeat, 'monthly');
  });
  it('a silly interval is ignored rather than sent to the backend', () => {
    assert.equal(parse('every 0 min thing').repeat, 'none');
    assert.equal(parse('every 999999 min thing').repeat, 'none');
  });
});

describe('priority and titles', () => {
  it('!! and !high', () => {
    assert.equal(parse('water plants !!').priority, 'high');
    assert.equal(parse('!high water plants').priority, 'high');
    assert.equal(parse('water plants !!').title, 'Water plants');
  });
  it('!low', () => {
    assert.equal(parse('!low sort desk').priority, 'low');
  });
  it('no time at all', () => {
    const p = parse('just a title');
    assert.equal(p.dueAt, null);
    assert.equal(p.repeat, 'none');
    assert.equal(p.priority, 'normal');
    assert.equal(p.title, 'Just a title');
  });
  it('digits that are not times stay in the title', () => {
    const p = parse('call 555-1234 tomorrow 4pm');
    assert.equal(p.title, 'Call 555-1234');
    assert.equal(p.dueAt, at(2026, 10, 9, 16, 0));
  });
  it('strips "remind me to" and stranded connectors', () => {
    assert.equal(parse('remind me to call mom at 5pm').title, 'Call mom');
    assert.equal(parse('call mom at').title, 'Call mom');
  });
  it('does not eat legitimate leading words', () => {
    assert.equal(parse('On call rota tomorrow 9am').title, 'On call rota');
  });
  it('does not mistake words for months or days', () => {
    assert.equal(parse('buy 2 mars bars').dueAt, null);
    assert.equal(parse('sun cream').dueAt, null);
    assert.equal(parse('may I ask').dueAt, null);
  });
  it('an empty string', () => {
    assert.deepEqual(parse('   '), { title: '', dueAt: null, repeat: 'none', priority: 'normal' });
  });
});
