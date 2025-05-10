# Simple-DcBot-Virtual-Bank-Backend

This is the backend for a **Simple DcBot Virtual Bank**. It provides basic modules for managing virtual card information, handling transactions, and connecting with different platforms via tokens. The backend is designed to integrate with a bot system for financial operations.

## Project Structure

The project is structured into multiple crates to modularize the components:

* **`function`**: Contains business logic and core functionalities.
* **`handler`**: Responsible for handling requests and interactions.
* **`stock`**: Manages stock and financial data.
* **`structure`**: Contains the data structures and models used throughout the project.

## card.json Schema

The `card.json` file is used to store user card data, connection tokens, and transaction history. The structure looks like this:

```json
{
  "XXXXXXXXXXXXXXXXXXX": {
    "card_holder": "XXXXXXXXXXXXXXXXXXX",
    "card_number": XXXXXXXXXXXXXXXX,
    "good_thru": "XXXX",
    "verify_number": "XXX",
    "balance": "0",
    "stock": null,
    "connection": {
      "Platform name": [
        {
          "target": "Platform name",
          "token": ""
        }
      ]
    },
    "transaction": null
  }
}
```

### Key Fields:

* **`card_holder`**: The name of the cardholder.
* **`card_number`**: The credit/debit card number.
* **`good_thru`**: The expiration date of the card.
* **`verify_number`**: The card's verification number (CVV).
* **`balance`**: The current balance on the card.
* **`stock`**: Stock data related to the card (can be `null` if not used).
* **`connection`**: A mapping of platforms that the card is connected to, with each platform containing a `token` for authentication.
* **`transaction`**: The history or state of transactions associated with this card.

## Setup

1. Clone this repository:

   ```bash
   git clone https://github.com/catroll1321/Simple-DcBot-Virtual-Bank-Backend.git
   cd Simple-DcBot-Virtual-Bank-Backend
   ```

2. Build the project using Cargo:

   ```bash
   cargo build
   ```

3. Run the backend:

   ```bash
   cargo run
   ```

## Dependencies

* Rust (latest stable version)
* `Cargo.toml` defines all necessary dependencies for building and running the project.
