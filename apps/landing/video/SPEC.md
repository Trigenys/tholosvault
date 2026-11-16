GOAL:    Un décisionnaire comprend en 65 s pourquoi « monter » une archive découpée bat « extraire », et ce que la découpe unique apporte (compression, vérification, réparation, delta).
OUTPUT:  HyperFrames, 1920 x 1080, 30 fps
TIME:    65 s x 30 = 1950 frames ; musique : lit calme, ~90 BPM, 20 frames par temps
LOOK:    ground #0B1220 uni (0 % grain), ink #EEF2F7, accent #F5B400 sur un mot par ligne et sur la parité ;
         rouge #E5484D réservé à la corruption ; morceaux : 8 teintes désaturées (#4C7BD9 #5BB98C #B08BE6 #E0905A #4FB3C9 #C9A54F #7A8FE0 #D97BA6)
         display Inter 700 54 px tracking -2 %, héros fin 88 px ; empreintes JetBrains Mono 22 px ; pas d'ombre, pas de vignette
OBJECTS: une boîte d'archive (corps + couvercle rabattable + étiquette mono), des fichiers (rectangles 44x56), une barre de disque,
         un chrono mono, un câble + prise, une arborescence de fichiers, un ruban de données découpé en morceaux de largeurs inégales,
         un arbre de Merkle (noeuds mono reliés par lignes SVG), des blocs de parité ambre, deux rangées v1/v2
MOTION:  arrivée expo.out 0.5 s ; sortie power3.in 0.25 s ; lignes mot à mot blur 6->0 en 0.13 s, 0.067 s d'écart ; staggers 40-80 ms ;
         coupes par échange flou sur une même scène (le fond ne coupe jamais)
SHOTS:   0-8    boîte fermée -> couvercle ouvert, 12 fichiers tombent vers le disque, barre 40->100 %, chrono 0:00->15:00, badge « disque plein »
         8-15   même boîte, câble arrive de la gauche, prise, lettre V: et arborescence en 0,4 s
         15-25  ruban découpé en 8 morceaux inégaux ; 2e rangée avec insertion ambre au début : seul le 1er change, 7 égalités
         25-34  les 8 morceaux rétrécissent (zstd) ; un morceau s'isole ; deux chronos : 122 ms vs 0,1 ms
         34-43  empreintes sous chaque morceau, arbre 8-4-2-1 ; morceau 5 rougit, chemin rouge jusqu'à la racine
         43-51  2 blocs de parité ambre ; le morceau rouge s'éteint, flèches convergentes, il se reconstruit
         51-58  rangées v1 et v2, 2 morceaux sur 10 changent, seuls ces deux descendent
         58-65  ligne héros « Une seule découpe sert à tout. », puis la liste et la devise, tenue 3 s
BANNED:  puces, icônes dans des cercles, trois cartes en ligne, diagramme complet d'un coup, titres qui dérivent après atterrissage,
         fondu + glissé identique partout, boucles qui respirent, lueur
QA:      f 150 (boîte ouverte, fichiers en vol), f 400 (arborescence), f 660 (8 morceaux + 2e rangée), f 900 (chronos),
         f 1200 (chemin rouge), f 1440 (reconstruction), f 1650 (deux descentes), f 1900 (ligne finale)
