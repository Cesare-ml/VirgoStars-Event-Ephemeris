// Test-only stand-in for the *external* private backend's position function.
module.exports = {
  nomeToIDElementoCeleste: { Sun: 0, Moon: 1, Mars: 4 },
  getPianetiCaseByNomiElCelesti(date, _longitude, _latitude, bodies) {
    if (typeof date !== 'string' || !date.includes(' ')) {
      throw new Error('unexpected UTC date conversion');
    }
    const planets = {};
    for (const body of bodies) {
      planets[body] = { longitude: body === 'Moon' ? 125 : 123, longitudeSpeed: 2 };
    }
    return { planets };
  },
};
