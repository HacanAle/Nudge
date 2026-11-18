// Run with: npm test
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { meetingLink, shortLink, splitLinks } from './links.ts';

describe('splitLinks', () => {
  it('leaves plain text alone', () => {
    assert.deepEqual(splitLinks('Room 4B. Bring the mockups.'), [{ text: 'Room 4B. Bring the mockups.' }]);
  });
  it('finds links between text, without the full stop after them', () => {
    assert.deepEqual(splitLinks('Agenda: https://example.com/agenda. Be early'), [
      { text: 'Agenda: ' },
      { text: 'https://example.com/agenda', href: 'https://example.com/agenda' },
      { text: '. Be early' }
    ]);
  });
  it('adds https to www links and keeps mailto', () => {
    const parts = splitLinks('see www.example.com or mailto:a@b.com');
    assert.equal(parts[1].href, 'https://www.example.com');
    assert.equal(parts[3].href, 'mailto:a@b.com');
  });
  it('keeps brackets that belong to the link, drops ones that wrap it', () => {
    assert.equal(splitLinks('https://en.wikipedia.org/wiki/Foo_(bar)')[0].href, 'https://en.wikipedia.org/wiki/Foo_(bar)');
    assert.equal(splitLinks('(https://example.com/x)')[1].href, 'https://example.com/x');
  });
  it('keeps line breaks in the text around links', () => {
    const parts = splitLinks('Line one\nhttps://a.example/x\nLine three');
    assert.equal(parts.map((p) => p.text).join(''), 'Line one\nhttps://a.example/x\nLine three');
  });
  it('ignores a bare scheme', () => {
    assert.deepEqual(splitLinks('type https:// first'), [{ text: 'type https:// first' }]);
  });
});

describe('meetingLink', () => {
  it('finds Teams, Zoom and Meet links', () => {
    assert.equal(meetingLink('Join: https://teams.microsoft.com/l/meetup-join/19%3a'), 'https://teams.microsoft.com/l/meetup-join/19%3a');
    assert.equal(meetingLink('https://us02web.zoom.us/j/123?pwd=x'), 'https://us02web.zoom.us/j/123?pwd=x');
    assert.equal(meetingLink('docs https://example.com then https://meet.google.com/abc-defg-hij'), 'https://meet.google.com/abc-defg-hij');
  });
  it('ignores other links and look-alike hosts', () => {
    assert.equal(meetingLink('https://example.com/zoom.us'), null);
    assert.equal(meetingLink('https://notzoom.us/j/1'), null);
    assert.equal(meetingLink('no links here'), null);
  });
});

describe('shortLink', () => {
  it('drops the scheme and shortens long links', () => {
    assert.equal(shortLink('https://www.example.com/'), 'example.com');
    assert.equal(shortLink('mailto:a@b.com'), 'a@b.com');
    const long = shortLink('https://teams.microsoft.com/l/meetup-join/19%3ameeting_abcdefghijklmnop');
    assert.equal(long.length, 44);
    assert.ok(long.endsWith('…'));
  });
});
