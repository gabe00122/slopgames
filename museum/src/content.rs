//! What the museum shows: galleries and exhibits, with their texts.

use bevy::color::Color;

pub struct Gallery {
    pub name: &'static str,
    pub era: &'static str,
    pub intro: &'static str,
    /// Which side of the octagonal rotunda the gallery opens from (0 = north,
    /// increasing clockwise seen from above).
    pub side: usize,
    pub wall: Color,
    pub accent: Color,
    pub length: f32,
    pub height: f32,
}

/// Where an exhibit stands inside its gallery. Distances are meters from the
/// gallery entrance.
#[derive(Clone, Copy)]
pub enum Slot {
    /// On the left wall side, facing into the aisle.
    Left(f32),
    Right(f32),
    /// Centered at the far end, facing the entrance.
    End,
    /// Suspended from the ceiling at (x, along, height).
    Hang(f32, f32, f32),
    /// The rotunda centerpiece.
    Center,
}

#[derive(Clone, Copy)]
pub enum Mount {
    /// Rectangular plinth of (width, height, depth).
    Plinth(f32, f32, f32),
    /// Round pedestal of (radius, height).
    Round(f32, f32),
    /// Low platform at floor level of (width, depth).
    Floor(f32, f32),
    /// Hangs from the ceiling on cables.
    Cables,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Model {
    Armillary,
    HandAxe,
    Fire,
    CaveArt,
    Wheat,
    Wheel,
    Cuneiform,
    Pyramid,
    Platonic,
    Parthenon,
    Antikythera,
    PrintingPress,
    Orrery,
    Telescope,
    Ship,
    NewtonCannon,
    WrightFlyer,
    Sputnik,
    SaturnV,
    Voyager,
    Jwst,
    SteamEngine,
    Locomotive,
    PeriodicTable,
    LightBulb,
    Radio,
    Microscope,
    Vaccine,
    TreeOfLife,
    Penicillin,
    Dna,
    AnalyticalEngine,
    Eniac,
    Transistor,
    Microchip,
    Web,
}

pub struct Exhibit {
    /// Index into [`GALLERIES`], or `None` for the rotunda.
    pub gallery: Option<usize>,
    pub title: &'static str,
    pub date: &'static str,
    pub tagline: &'static str,
    pub blurb: &'static str,
    pub model: Model,
    pub slot: Slot,
    pub mount: Mount,
}

pub const MUSEUM_NAME: &str = "The Museum of Human Achievement";

pub const WELCOME: &str = "Seven galleries open from the rotunda ahead, tracing how people learned to make tools, \
build cities, read the laws of nature, harness energy, heal the sick, reach space and connect the world. \
Walk up to any exhibit to learn its story.";

/// One quotation for each gallery, in gallery order.
pub const QUOTES: [(&str, &str); 7] = [
    (
        "Man is a tool-using animal. Without tools he is nothing, with tools he is all.",
        "Thomas Carlyle, 1834",
    ),
    ("Give me a place to stand, and I shall move the Earth.", "Archimedes"),
    (
        "If I have seen further, it is by standing on the shoulders of giants.",
        "Isaac Newton, 1675",
    ),
    (
        "That's one small step for a man, one giant leap for mankind.",
        "Neil Armstrong, 1969",
    ),
    (
        "I sell here, Sir, what all the world desires to have: Power.",
        "Matthew Boulton, 1776",
    ),
    (
        "Nothing in biology makes sense except in the light of evolution.",
        "Theodosius Dobzhansky, 1973",
    ),
    (
        "The Analytical Engine weaves algebraical patterns just as the Jacquard loom weaves flowers and leaves.",
        "Ada Lovelace, 1843",
    ),
];

fn srgb(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn galleries() -> [Gallery; 7] {
    [
        Gallery {
            name: "Dawn of Humanity",
            era: "3.3 million – 5,000 years ago",
            intro: "Long before cities or writing, our ancestors learned to shape stone, tame fire, paint on the walls \
of caves and coax food from the soil. Every later achievement rests on these foundations.",
            side: 5,
            wall: srgb(0x8a4e34),
            accent: srgb(0xe0a050),
            length: 30.0,
            height: 7.0,
        },
        Gallery {
            name: "Ancient Wonders",
            era: "3200 BCE – 500 CE",
            intro: "With cities came writing, mathematics, monumental architecture and new ideas about how people \
should govern themselves. The ancient world laid down the tools of civilization.",
            side: 6,
            wall: srgb(0x243f6e),
            accent: srgb(0xe2bd62),
            length: 30.0,
            height: 7.0,
        },
        Gallery {
            name: "The Age of Discovery",
            era: "1000 – 1750",
            intro: "Printing spread ideas faster than ever before, navigators circled the globe, and a new method \
of inquiry, science, began to reveal the laws that govern the heavens and the Earth.",
            side: 7,
            wall: srgb(0x2a5240),
            accent: srgb(0xd4b06a),
            length: 30.0,
            height: 7.0,
        },
        Gallery {
            name: "Flight & Space",
            era: "1903 – today",
            intro: "Within a single lifetime, humanity went from the first powered flight to footprints on the Moon. \
Our robotic explorers have since visited every planet and crossed into interstellar space.",
            side: 0,
            wall: srgb(0x151b2e),
            accent: srgb(0x8ab8ff),
            length: 40.0,
            height: 16.0,
        },
        Gallery {
            name: "Power & Industry",
            era: "1700 – 1900",
            intro: "Harnessing coal, steam and electricity multiplied the strength of human hands a thousandfold. \
In little more than a century, engines, railways and power grids transformed how people lived and worked.",
            side: 1,
            wall: srgb(0x6a2c26),
            accent: srgb(0xf0955a),
            length: 30.0,
            height: 7.0,
        },
        Gallery {
            name: "Life & Medicine",
            era: "1670 – today",
            intro: "For most of history, nearly half of all children died before growing up. Microscopes, vaccines, \
antibiotics and the discovery of DNA changed that: global life expectancy has more than doubled since 1900.",
            side: 2,
            wall: srgb(0x255e5e),
            accent: srgb(0x8fe0c8),
            length: 30.0,
            height: 7.0,
        },
        Gallery {
            name: "The Information Age",
            era: "1837 – today",
            intro: "From brass gears to billions of transistors on a chip, machines that process information have \
transformed science, work and daily life, and connected most of humanity into a single network.",
            side: 3,
            wall: srgb(0x29233f),
            accent: srgb(0xb896ff),
            length: 30.0,
            height: 7.0,
        },
    ]
}

use Model::*;
use Mount::*;
use Slot::*;

const STD_PLINTH: Mount = Plinth(1.3, 0.9, 1.3);

pub fn exhibits() -> Vec<Exhibit> {
    let e = |gallery: Option<usize>, title, date, tagline, blurb, model, slot, mount| Exhibit {
        gallery,
        title,
        date,
        tagline,
        blurb,
        model,
        slot,
        mount,
    };
    vec![
        e(
            None,
            "The Armillary Sphere",
            "Curiosity, in every age",
            "\u{201c}We are a way for the cosmos to know itself.\u{201d} — Carl Sagan",
            "Every achievement in this museum began with someone asking a question. The armillary sphere, a set of \
rings modelling the great circles of the sky, was used from ancient Greece and Han China to Renaissance Europe to \
teach and calculate the motions of the heavens. Around you, seven galleries trace the story of human ingenuity.",
            Armillary,
            Center,
            Round(1.3, 1.1),
        ),
        // ---------------------------------------------------------------- Dawn
        e(
            Some(0),
            "Stone Tools",
            "c. 3.3 million years ago · Lomekwi, Kenya",
            "The first technology",
            "The oldest known stone tools, found at Lomekwi in Kenya, are older than our own genus. By 1.76 million \
years ago, Homo erectus was shaping symmetrical Acheulean hand axes like this one, a design so successful that it \
was used for more than a million years across Africa, Europe and Asia.",
            HandAxe,
            Left(6.0),
            STD_PLINTH,
        ),
        e(
            Some(0),
            "Control of Fire",
            "c. 1 million years ago · Wonderwerk Cave, South Africa",
            "Warmth, light and cooked food",
            "Burnt bone and plant ash in Wonderwerk Cave show that our ancestors were using fire a million years ago. \
Fire kept predators at bay, lengthened the day and made food easier to digest, which may have helped fuel the \
evolution of larger brains.",
            Fire,
            Right(10.0),
            Floor(2.2, 2.2),
        ),
        e(
            Some(0),
            "Cave Art",
            "51,200 years ago · Sulawesi  ·  17,000 years ago · Lascaux",
            "The first images",
            "Hand stencils and animals in red ochre and charcoal appear on cave walls from Indonesia to France. A \
painting of a pig and human figures on Sulawesi is at least 51,200 years old. These images are among the earliest \
signs of symbolic thought: minds that could imagine, remember and share.",
            CaveArt,
            Left(15.0),
            Floor(3.2, 1.2),
        ),
        e(
            Some(0),
            "Agriculture",
            "c. 9500 BCE · The Fertile Crescent",
            "Planting the future",
            "In the hills of the Fertile Crescent, people began to sow and select wild wheat and barley. Farming arose \
independently in several regions, including China, Mesoamerica, the Andes and New Guinea. Reliable harvests made \
permanent villages possible, and eventually cities.",
            Wheat,
            Right(20.0),
            STD_PLINTH,
        ),
        e(
            Some(0),
            "The Wheel",
            "c. 3500 BCE · Mesopotamia and Europe",
            "Rolling forward",
            "The wheel appeared around 3500 BCE, probably first as the potter's wheel. The hard part was not the disc \
but the axle and hub, which had to fit precisely. Early wheels were solid, made from planks of wood pinned \
together like this three-part design, and spread rapidly across Eurasia.",
            Wheel,
            End,
            Plinth(1.8, 0.5, 1.4),
        ),
        // ------------------------------------------------------------- Ancient
        e(
            Some(1),
            "Writing",
            "c. 3200 BCE · Uruk, Mesopotamia",
            "Speech made permanent",
            "Sumerian scribes pressed wedge-shaped marks, called cuneiform, into wet clay: first to count grain and \
livestock, later to record laws, letters and literature such as the Epic of Gilgamesh. Writing was also invented \
independently in China and Mesoamerica, and perhaps in Egypt.",
            Cuneiform,
            Left(6.0),
            STD_PLINTH,
        ),
        e(
            Some(1),
            "The Great Pyramid",
            "c. 2560 BCE · Giza, Egypt",
            "The tallest structure on Earth for 3,800 years",
            "Built for the pharaoh Khufu from about 2.3 million blocks of stone, the Great Pyramid rose 146 metres and \
was aligned to the cardinal directions within a fraction of a degree. No taller structure was built for nearly \
four thousand years.",
            Pyramid,
            Right(10.0),
            Plinth(2.0, 0.6, 2.0),
        ),
        e(
            Some(1),
            "Euclid's Elements",
            "c. 300 BCE · Alexandria",
            "The art of proof",
            "Euclid gathered the geometry of his age into thirteen books, deriving hundreds of theorems from a handful \
of axioms. The final book proves that there are exactly five regular solids, the Platonic solids shown here. \
The Elements was used as a textbook for more than two thousand years.",
            Platonic,
            Left(15.0),
            STD_PLINTH,
        ),
        e(
            Some(1),
            "The Parthenon",
            "447–432 BCE · Athens",
            "Democracy in marble",
            "Built at the height of Athenian democracy, the Parthenon has almost no truly straight lines: its floor \
curves upward and its columns lean slightly inward so that it looks perfectly straight. Athens gave the world the \
idea that ordinary citizens could debate and vote on their own laws.",
            Parthenon,
            Right(20.0),
            Plinth(2.2, 0.7, 1.5),
        ),
        e(
            Some(1),
            "The Antikythera Mechanism",
            "2nd century BCE · Greece",
            "The first known analog computer",
            "Recovered from a Roman-era shipwreck in 1901, this corroded box of at least 30 bronze gears could predict \
the positions of the Sun and Moon, the Moon's phases, eclipses and the dates of the Olympic Games. Nothing of \
comparable complexity is known for more than a thousand years afterward.",
            Antikythera,
            End,
            Plinth(1.6, 1.0, 1.0),
        ),
        // ----------------------------------------------------------- Discovery
        e(
            Some(2),
            "Movable-Type Printing",
            "c. 1040 Bi Sheng · c. 1440 Johannes Gutenberg",
            "Knowledge for everyone",
            "Bi Sheng invented movable type in China around 1040. Gutenberg's combination of metal type, oil-based \
ink and a screw press made printing fast and cheap. By 1500, millions of books were circulating in Europe, fuelling \
the Renaissance, the Reformation and the Scientific Revolution.",
            PrintingPress,
            Left(6.0),
            Floor(1.8, 1.4),
        ),
        e(
            Some(2),
            "Heliocentrism",
            "1543 · Nicolaus Copernicus",
            "Moving the Earth",
            "In On the Revolutions of the Heavenly Spheres, Copernicus put the Sun, not the Earth, at the centre. \
Kepler showed that the planets move in ellipses, and Galileo's observations supported the new picture. This orrery \
shows the planets known at the time circling the Sun.",
            Orrery,
            Right(10.0),
            Round(0.8, 0.9),
        ),
        e(
            Some(2),
            "The Telescope",
            "1609 · Galileo Galilei",
            "Looking up",
            "Hearing of a Dutch spyglass, Galileo built his own and turned it to the sky. He saw mountains on the Moon, \
the phases of Venus, countless stars in the Milky Way and four moons circling Jupiter: proof that not everything \
revolves around the Earth.",
            Telescope,
            Left(15.0),
            Floor(1.6, 1.6),
        ),
        e(
            Some(2),
            "Circumnavigation",
            "1519–1522 · Magellan–Elcano expedition",
            "The world is round, and vast",
            "Five ships and about 270 crew left Spain. Three years later a single ship, the Victoria, returned under \
Juan Sebastián Elcano with 18 survivors. Their voyage proved that the oceans are connected and revealed the true, \
enormous size of the Earth.",
            Ship,
            Right(20.0),
            Plinth(1.8, 0.8, 1.0),
        ),
        e(
            Some(2),
            "Newton's Principia",
            "1687 · Isaac Newton",
            "One law for heaven and Earth",
            "Newton showed that the force that pulls an apple to the ground also holds the Moon in orbit. He imagined \
a cannon on a mountaintop: fire a ball fast enough and it falls around the Earth forever. His laws of motion and \
gravitation still guide spacecraft today.",
            NewtonCannon,
            End,
            Round(0.9, 0.9),
        ),
        // --------------------------------------------------------------- Space
        e(
            Some(3),
            "First Powered Flight",
            "17 December 1903 · Kitty Hawk, North Carolina",
            "Twelve seconds that changed the world",
            "Orville and Wilbur Wright, bicycle makers from Ohio, solved the problem of control that had defeated \
earlier pioneers. The Flyer's first flight lasted 12 seconds and covered 37 metres. Sixty-six years later, people \
walked on the Moon. (Half-scale model.)",
            WrightFlyer,
            Hang(0.0, 9.0, 6.5),
            Cables,
        ),
        e(
            Some(3),
            "Sputnik 1",
            "4 October 1957 · Baikonur",
            "The first artificial moon",
            "A polished metal sphere 58 cm across with four long antennas, Sputnik 1 circled the Earth every 96 \
minutes, broadcasting a simple radio beep that anyone could hear. It opened the Space Age.",
            Sputnik,
            Hang(-3.4, 17.0, 3.6),
            Cables,
        ),
        e(
            Some(3),
            "Voyager and the Golden Record",
            "Launched 1977",
            "A message in a bottle",
            "The twin Voyagers toured Jupiter, Saturn, Uranus and Neptune, and are now in interstellar space, the most \
distant objects humans have ever made. Each carries a gold-plated record of sounds, music and images from Earth: \
a greeting for anyone who might one day find it.",
            Voyager,
            Hang(3.4, 23.0, 5.0),
            Cables,
        ),
        e(
            Some(3),
            "Space Telescopes",
            "1990 Hubble · 2021 James Webb",
            "Seeing to the edge of time",
            "Above the blurring atmosphere, Hubble helped measure the expansion of the universe. Its successor, the \
James Webb Space Telescope, unfolds a 6.5-metre mirror of 18 gold-coated segments behind a sunshield the size of \
a tennis court, and sees galaxies from the universe's first few hundred million years.",
            Jwst,
            Left(29.0),
            Plinth(2.4, 0.6, 2.4),
        ),
        e(
            Some(3),
            "Apollo 11",
            "20 July 1969 · Sea of Tranquility",
            "One giant leap",
            "The Saturn V stood 110 metres tall and remains one of the most powerful rockets ever flown. Four days \
after launch, Neil Armstrong and Buzz Aldrin landed on the Moon while Michael Collins orbited above. Twelve people \
walked on the Moon between 1969 and 1972. (1:10 scale.)",
            SaturnV,
            End,
            Round(1.8, 0.5),
        ),
        // ------------------------------------------------------------ Industry
        e(
            Some(4),
            "The Steam Engine",
            "1712 Newcomen · 1776 James Watt",
            "Power on demand",
            "Thomas Newcomen's engine pumped water from mines but wasted most of its fuel. James Watt's separate \
condenser made steam engines far more efficient, and his rotary engines went on to drive mills and factories. \
The unit of power, the watt, bears his name.",
            SteamEngine,
            Left(6.0),
            Plinth(2.4, 0.4, 1.4),
        ),
        e(
            Some(4),
            "The Railway",
            "1829 · Stephenson's Rocket",
            "Distance conquered",
            "Robert Stephenson's Rocket won the Rainhill Trials at up to 48 km/h (30 mph), and its multi-tube boiler \
set the pattern for steam locomotives for more than a century. Railways shrank journeys of days into hours and \
led to standardised time.",
            Locomotive,
            Right(10.0),
            Plinth(2.6, 0.5, 1.2),
        ),
        e(
            Some(4),
            "The Periodic Table",
            "1869 · Dmitri Mendeleev",
            "The alphabet of matter",
            "Mendeleev arranged the 63 known elements by atomic weight and chemical behaviour, and left gaps: he \
predicted the properties of elements no one had yet found. Gallium, scandium and germanium were soon discovered, \
just as he described. Today the table holds 118 elements.",
            PeriodicTable,
            Left(15.0),
            Floor(3.4, 0.8),
        ),
        e(
            Some(4),
            "Electric Light",
            "1879 · Joseph Swan and Thomas Edison",
            "Night into day",
            "Swan in Britain and Edison in the United States developed practical incandescent bulbs with carbon \
filaments glowing in a vacuum. Edison's Pearl Street Station began selling electricity in 1882, starting the \
electrification of the world.",
            LightBulb,
            Right(20.0),
            Round(0.7, 0.9),
        ),
        e(
            Some(4),
            "Radio",
            "1895–1901 · Guglielmo Marconi",
            "Voices through the air",
            "Maxwell predicted electromagnetic waves and Hertz produced them. Marconi turned them into a way to send \
messages without wires, across the Atlantic by 1901. Radio saved lives at sea and led to broadcasting, radar, \
television, Wi-Fi and mobile phones.",
            Radio,
            End,
            Round(1.2, 0.3),
        ),
        // ---------------------------------------------------------------- Life
        e(
            Some(5),
            "The Microscope",
            "1676 · Antonie van Leeuwenhoek",
            "A hidden world",
            "With tiny hand-ground lenses, the Dutch draper Antonie van Leeuwenhoek became the first person to see \
bacteria and single-celled 'animalcules'. Two centuries later, Louis Pasteur and Robert Koch showed that microbes \
cause disease, and germ theory became a foundation of modern medicine.",
            Microscope,
            Left(6.0),
            STD_PLINTH,
        ),
        e(
            Some(5),
            "Vaccination",
            "1796 Edward Jenner · 1980 smallpox eradicated",
            "A disease erased",
            "Jenner showed that inoculation with cowpox protected against smallpox, which killed an estimated 300 \
million people in the 20th century alone. A worldwide vaccination campaign eradicated smallpox in 1980, the first \
human disease ever wiped out.",
            Vaccine,
            Right(10.0),
            Plinth(1.2, 1.0, 1.2),
        ),
        e(
            Some(5),
            "Evolution by Natural Selection",
            "1859 · Charles Darwin and Alfred Russel Wallace",
            "Endless forms most beautiful",
            "Darwin's On the Origin of Species explained how natural selection, acting over immense spans of time, \
produces the diversity of life. Wallace reached the same idea independently. Every living thing shares a common \
ancestor; this bronze tree traces the branching.",
            TreeOfLife,
            Left(15.0),
            Round(0.9, 0.4),
        ),
        e(
            Some(5),
            "Penicillin",
            "1928 · Alexander Fleming",
            "The mould that saved millions",
            "Fleming noticed that a stray Penicillium mould had killed the bacteria around it in a culture dish. \
Howard Florey, Ernst Chain and Norman Heatley turned it into a medicine in the 1940s, opening the age of \
antibiotics, which have saved hundreds of millions of lives.",
            Penicillin,
            Right(20.0),
            STD_PLINTH,
        ),
        e(
            Some(5),
            "The Double Helix",
            "1953 · Franklin, Wilkins, Watson and Crick",
            "The code of life",
            "X-ray images from Rosalind Franklin's lab revealed a helix, and James Watson and Francis Crick built the \
model: two strands of paired bases, A with T and G with C, carrying the instructions for every living thing. In \
2003 the Human Genome Project read all three billion letters of our own DNA.",
            Dna,
            End,
            Round(1.0, 0.4),
        ),
        // --------------------------------------------------------- Information
        e(
            Some(6),
            "The Analytical Engine",
            "1837 Charles Babbage · 1843 Ada Lovelace",
            "The first computer program",
            "Babbage designed a general-purpose mechanical computer with a memory, a processing 'mill' and \
punched-card input. It was never completed, but Ada Lovelace wrote an algorithm for it to compute Bernoulli \
numbers, and foresaw that such machines might one day compose music.",
            AnalyticalEngine,
            Left(6.0),
            Plinth(1.8, 0.5, 1.2),
        ),
        e(
            Some(6),
            "ENIAC",
            "1945 · University of Pennsylvania",
            "The electronic brain",
            "With 17,468 vacuum tubes and weighing 27 tonnes, ENIAC could perform 5,000 additions a second. It was \
programmed by six women: Kay McNulty, Jean Jennings, Betty Snyder, Marlyn Wescoff, Fran Bilas and Ruth \
Lichterman, who worked out how to program it largely from its wiring diagrams.",
            Eniac,
            Right(10.0),
            Floor(3.6, 1.2),
        ),
        e(
            Some(6),
            "The Transistor",
            "1947 · Bell Labs",
            "The switch that changed everything",
            "John Bardeen and Walter Brattain, working in William Shockley's group, pressed two strips of gold foil \
onto a slab of germanium and made it amplify a signal. Small, cool and reliable, transistors replaced vacuum tubes; \
they are the most manufactured objects in history. (Shown enlarged.)",
            Transistor,
            Left(15.0),
            STD_PLINTH,
        ),
        e(
            Some(6),
            "The Microprocessor",
            "1971 · Intel 4004",
            "A computer on a chip",
            "Federico Faggin, Ted Hoff, Stanley Mazor and Masatoshi Shima put an entire processor onto one chip of \
silicon with 2,300 transistors. Following Moore's law, the largest chips today hold more than a hundred billion. \
(Shown enlarged.)",
            Microchip,
            Right(20.0),
            STD_PLINTH,
        ),
        e(
            Some(6),
            "The World Wide Web",
            "1989 · Tim Berners-Lee, CERN",
            "Everything, everywhere",
            "Built on the internet (ARPANET, 1969; TCP/IP, 1983), Tim Berners-Lee's World Wide Web linked documents \
across computers with hyperlinks. CERN made it free for anyone to use in 1993. Today more than five billion people \
are online.",
            Web,
            End,
            Round(1.2, 0.5),
        ),
    ]
}
