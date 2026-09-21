import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.Reference;
public class PhysicsFactoryCallAssembly extends GhidraScript {
 @Override public void run()throws Exception{
  Function f=getFunctionAt(toAddr(0x0047ECB0L)); if(f==null)f=getFunctionContaining(toAddr(0x0047ECB0L));
  InstructionIterator it=currentProgram.getListing().getInstructions(f.getBody(),true);
  while(it.hasNext()){Instruction ins=it.next();boolean hit=false;for(Reference ref:ins.getReferencesFrom())if(ref.getToAddress().equals(toAddr(0x004E803CL)))hit=true;
   if(hit){Instruction cur=ins;for(int i=0;i<12;i++)cur=cur.getPrevious();for(int i=0;i<22&&cur!=null;i++,cur=cur.getNext())println(cur.getAddress()+"  "+cur);}
  }
 }
}