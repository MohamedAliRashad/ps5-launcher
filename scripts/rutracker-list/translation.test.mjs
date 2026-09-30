import assert from 'node:assert/strict';
import { test } from 'node:test';
import { englishCatalog, knownEnglish, numericTokens, prepareCatalog, protectTechnical, translationInputs } from './english.mjs';

test('English catalog preserves technical data and translates labels and language lists', () => {
  const original = { topics: [{ id: '100', title: '[PS5] Game [Лицензия]', magnet: 'magnet:?xt=urn:btih:abc',
    url: 'https://example.com/Русский', game_info: { title_id: 'PPSA12345',
      fields: { 'Язык интерфейса': 'Русский, Английский', 'Тип издания': 'Лицензия', 'Возраст': '16+ОписаниеLong marketing text' },
      interface_languages: 'Русский, Английский', edition: 'Лицензия', age_rating: '16+ОписаниеLong marketing text',
      minimum_firmware: '10.хх', name: 'Game', genre: 'Экшен', release_summary: 'Old summary',
    } }] };
  const prepared = prepareCatalog(original);
  assert.equal(prepared.topics[0].game_info.age_rating, '16+');
  assert.deepEqual(translationInputs(prepared), []);
  const translated = englishCatalog(prepared, {});
  const topic = translated.topics[0];
  assert.equal(topic.title, '[PS5] Game [Retail]');
  assert.equal(topic.url, original.topics[0].url);
  assert.equal(topic.magnet, original.topics[0].magnet);
  assert.equal(topic.game_info.fields['Interface languages'], 'Russian, English');
  assert.equal(topic.game_info.minimum_firmware, '10.xx');
  assert.match(topic.game_info.release_summary, /Action/);
  assert.equal(original.topics[0].game_info.edition, 'Лицензия');
});

test('translation maps retain distinct values on key collisions and repair firmware boundaries', () => {
  const prepared = prepareCatalog({ topics: [{ game_info: { fields: {
    'Формат образа': 'FPKGМинимальная версия прошивки: 4.xx+',
    'Возраст': '16+', 'Возрастное ограничение': '18+', 'Жанр': 'Ролевые приключения',
  }, format: 'FPKGМинимальная версия прошивки: 4.xx+' } }] });
  assert.equal(prepared.topics[0].game_info.format, 'FPKG');
  assert.equal(prepared.topics[0].game_info.minimum_firmware, '4.xx+');
  const translated = englishCatalog(prepared, { 'Ролевые Adventure': 'Role-playing adventure' });
  assert.equal(translated.topics[0].game_info.fields['Age rating'], '16+');
  assert.equal(translated.topics[0].game_info.fields['Age rating (2)'], '18+');
  assert.equal(knownEnglish('Китайский (традиционное письмо)'), 'Chinese (Traditional)');
  assert.equal(knownEnglish('Инструкция по запуску на ПО 4.03'), 'Инструкция по запуску на firmware 4.03');
});

test('machine translation cannot reformat version numbers, dates or tool names', () => {
  const input = 'Version 1.200.007 dated 13.12.2024; Y2JB 2.0; firmware 4.xx+';
  const protectedText = protectTechnical(input);
  assert.equal(protectedText.restore(protectedText.value), input);
  assert.deepEqual(numericTokens(input), ['1.200.007', '13.12.2024', '2', '2.0', '4.xx']);
  assert.throws(() => protectedText.restore('A translation that dropped a token'), /placeholder lost/);
});