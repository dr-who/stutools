#include <mysql.h>
#include <stdio.h>
#include <stdlib.h>

void finish_with_error(MYSQL *con)
{
  fprintf(stderr, "%s\n", mysql_error(con));
  mysql_close(con);
  exit(1);
}

int main(int argc, char **argv)
{
  MYSQL *con = mysql_init(NULL);

  if (con == NULL)
  {
      fprintf(stderr, "%s\n", mysql_error(con));
      exit(1);
  }

  if (mysql_real_connect(con, "localhost", "root", "",
          "testdb", 0, NULL, 0) == NULL)
  {
      finish_with_error(con);
  }

  //  if (mysql_query(con, "DROP TABLE IF EXISTS cars")) {
  //      finish_with_error(con);
//  }

  if (mysql_query(con, "CREATE TABLE IF NOT EXISTS cars(id INT PRIMARY KEY AUTO_INCREMENT, name VARCHAR(255), email VARCHAR(255))")) {
      finish_with_error(con);
  }

  if (mysql_query(con, "CREATE TABLE IF NOT EXISTS sessions(id INT PRIMARY KEY AUTO_INCREMENT, session VARCHAR(40) unique, customerid int)")) {
      finish_with_error(con);
  }

  if (mysql_query(con, "INSERT INTO cars VALUES(NULL,'Audi','stuart@v.v')")) {
      finish_with_error(con);
  }

  if (mysql_query(con, "INSERT INTO cars VALUES(NULL,'Mercedes','test@test.com')")) {
      finish_with_error(con);
  }

  if (mysql_query(con, "INSERT INTO cars VALUES(NULL,'Skoda',9000)")) {
      finish_with_error(con);
  }

  if (mysql_query(con, "INSERT INTO cars VALUES(NULL,'Volvo',29000)")) {
      finish_with_error(con);
  }

  if (mysql_query(con, "INSERT INTO cars VALUES(NULL,'Bentley',350000)")) {
      finish_with_error(con);
  }

  if (mysql_query(con, "INSERT INTO cars VALUES(NULL,'Citroen',21000)")) {
      finish_with_error(con);
  }

  if (mysql_query(con, "INSERT INTO cars VALUES(NULL,'Hummer',41400)")) {
      finish_with_error(con);
  }

  if (mysql_query(con, "INSERT INTO cars VALUES(NULL,'Volkswagen',21600)")) {
      finish_with_error(con);
  }

  int id = mysql_insert_id(con);

  printf("The last inserted row id is: %d\n", id);
  
  mysql_close(con);
  exit(0);
}
