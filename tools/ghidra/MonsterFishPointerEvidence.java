import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSetView;
public class MonsterFishPointerEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  byte[] p={(byte)0xC4,(byte)0x9E,(byte)0x61,0};
  Address cur=currentProgram.getMinAddress();
  while(cur!=null){
   Address found=find(cur,p);
   if(found==null)break;
   println("PTR@"+found);
   long base=found.getOffset()-4;
   for(int off=-0x10;off<=0x20;off+=4){
    Address a=toAddr(found.getOffset()+off);
    println(String.format(" %s = 0x%08X",a,Integer.toUnsignedLong(getInt(a))));
   }
   cur=found.add(1);
  }
 }
}