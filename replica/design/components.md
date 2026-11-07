# Wii OS Web : composants

Les variables de `tokens.css` sont chargées avant `app.css` et sont aussi servies depuis `/assets/tokens.css` par le binaire.

- **Navigation latérale / barre basse** : quatre routes; état `aria-current`; lien clavier natif; réduit à une barre tactile en mobile.
- **Bouton** : primaire, clair, texte, icône; tailles tactiles ≥ 31 px; focus visible, disabled, loading conservant le label accessible.
- **Carte chaîne** : vignette générée localement, titre échappé, badge statut, lancement disabled si contenu absent; pas d’image distante.
- **Formulaires** : labels visibles, contraintes HTML, pending désactivé, erreur en toast/texte `role=alert`.
- **Salon** : compteur présent/max, rejoindre/quitter via WebSocket, aucun faux indicateur de relais NetPlay.
- **Dialogue session** : mot de passe, erreur visible, secret effacé après échange, cookie `HttpOnly`.
- **Toast** : région `aria-live`, succès et erreur, se supprime automatiquement.
- **Réduction de mouvement** : respecte `prefers-reduced-motion`; styles de focus visibles et contrôles HTML natifs.
