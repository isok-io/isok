1-Ajout de check redis:
    - Deux tests a prévoir pour un même Redis (uniquement un changement de conf, et un "parametre" par check)
    - Des sondes end-to-end (accès au redis à travers un Load Balencer)
    - Des sondes optionelle accès direct

2-Les sondes:
    - Ping fonctionelle (redis)
    - Scénario de test de base (lecture d'une clé static potentiellement set par le user, écriture d'une clé puis lecture et suppression de celle-ci). check optionel, chaque commande doit gérer ses erreurs.
    - Scénario de test custom (éxecution de plusieurs commandes Redis données par le client). Check optionnelle, gestion unitaire par commande des erreurs
    - Pour chaque sondes extraction de METRICS, Temps d'éxécutions, erreurs, et d'autres à définir.

3- Configuration des Checks:
    - La fréquence (temps entre chaque check) doit étre configurable par Redis avec une valeur minimum (10 Seconds) et une valeur par défaut (60 Seconds)
    
4- Notifications des erreurs:   
    - On vas avoir une erreur lorsque un pourcentage de check d'une meme sonde échoue
    - Si N checks d'une meme sondes sont en erreur
    - Pour la Metric de temps d'éxécution on peux considerer une erreur si celui ci dépasse un certain seuil défaut (3 seconds)

5- Notion de source des checks:
    - Définir une source de checks qui soit l'instance. Ce qui permet de rendre configurable la notion d'erreur ou il faudrait que N sources soient en erreur (defaut 2 instances).

Tech: 
 - Notion de thread par sonde ?
 - Comment sont persistés les états (entre chaque check)
 - Comment tu démarres une sonde (le trigger)
 - Comment on met à jour une sonde (nouveau context)
 - Comment sont gérer les redemarrages
 - Notion des timeouts de chaque checks (potentiellement regarder pour mettre en place une sorte de backhoff strategy)
 